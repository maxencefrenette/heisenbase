/// Contains the code to index PGN files to find the most common material keys.
/// Stores the results in Parquet files.
use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::File,
    io,
    io::Read,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use flate2::read::MultiGzDecoder;
use heisenbase::material_key::MaterialKey;
use heisenbase::position_indexer::PositionIndexer;
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use polars::{
    error::PolarsError,
    prelude::{
        DataFrame, DataType, IntoLazy, LazyFrame, NamedFrom, ParquetWriter, Series, col, lit,
    },
};
use shakmaty::{CastlingMode, Chess, Position, fen::Fen};

const PGN_ROOT: &str = "./data/fishtest_pgns";
const MAX_NON_PAWN: u32 = 5;
const ILLEGAL_MOVE_PREFIX: &str = "illegal move:";
const INVALID_FEN_TAG_PREFIX: &str = "invalid FEN tag:";
const INVALID_FEN_POSITION_PREFIX: &str = "invalid FEN position:";
const CORRUPT_GZIP_PREFIX: &str = "corrupt gzip stream";
pub const RAW_PARQUET_PATH: &str = "./data/pgn_index_raw.parquet";
pub const PARQUET_PATH: &str = "./data/pgn_index.parquet";

pub fn run_stage1() -> io::Result<()> {
    let mut files = Vec::new();
    collect_pgn_files(Path::new(PGN_ROOT), &mut files)?;
    files.sort();

    let mut counts_games: HashMap<MaterialKey, u64> = HashMap::new();
    let mut counts_positions: HashMap<MaterialKey, u64> = HashMap::new();
    let mut total_games: u64 = 0;
    let mut total_positions: u64 = 0;

    for path in files {
        println!("Processing {}", path.display());
        let file = File::open(&path)?;
        let game_count = if is_gz(&path) {
            process_reader(
                MultiGzDecoder::new(file),
                &mut counts_games,
                &mut counts_positions,
                &mut total_positions,
                &path,
            )?
        } else {
            process_reader(
                file,
                &mut counts_games,
                &mut counts_positions,
                &mut total_positions,
                &path,
            )?
        };
        total_games += game_count;
    }

    println!("Processed {total_games} games.");

    let mut entries: Vec<_> = counts_games.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    write_raw_index(&entries, &counts_positions, total_games, total_positions)?;

    Ok(())
}

pub fn run_stage2() -> io::Result<()> {
    let mut df = LazyFrame::scan_parquet(RAW_PARQUET_PATH, Default::default())
        .map_err(polars_to_io_error)?
        .filter(col("num_games").gt(1))
        .collect()
        .map_err(polars_to_io_error)?;

    let sizes = material_key_sizes(&df)?;
    df.with_column(Series::new("material_key_size", sizes))
        .map_err(polars_to_io_error)?;

    let mut df = df
        .lazy()
        .with_columns([
            (lit(1_000_000_000f64) * col("num_positions").cast(DataType::Float64)
                / col("total_positions").cast(DataType::Float64)
                / col("material_key_size").cast(DataType::Float64))
            .alias("utility"),
        ])
        .collect()
        .map_err(polars_to_io_error)?;

    if let Some(parent) = Path::new(PARQUET_PATH).parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(PARQUET_PATH)?;
    ParquetWriter::new(file)
        .finish(&mut df)
        .map_err(polars_to_io_error)?;

    Ok(())
}

fn process_reader<R: Read>(
    reader: R,
    counts_games: &mut HashMap<MaterialKey, u64>,
    counts_positions: &mut HashMap<MaterialKey, u64>,
    total_positions: &mut u64,
    path: &Path,
) -> io::Result<u64> {
    let mut reader = Reader::new(reader);
    let mut visitor = IndexVisitor {
        counts_games,
        counts_positions,
        total_positions,
        games: 0,
    };
    let mut skipped = SkipStats::default();
    loop {
        match reader.read_game(&mut visitor) {
            Ok(Some(result)) => match result {
                Ok(()) => {}
                Err(err) => {
                    if !classify_skip_error(&err, &mut skipped) {
                        return Err(err);
                    }
                }
            },
            Ok(None) => break,
            Err(err) if is_corrupt_gzip_error(&err) => {
                eprintln!(
                    "Stopped early due to corrupt gzip data in {}: {err}",
                    path.display()
                );
                break;
            }
            Err(err) => return Err(err),
        }
    }
    skipped.report(path);
    Ok(visitor.games)
}

fn write_raw_index(
    entries: &[(MaterialKey, u64)],
    counts_positions: &HashMap<MaterialKey, u64>,
    total_games: u64,
    total_positions: u64,
) -> io::Result<()> {
    let mut material_keys = Vec::with_capacity(entries.len());
    let mut counts_games = Vec::with_capacity(entries.len());
    let mut counts_positions_vec = Vec::with_capacity(entries.len());
    let mut total_games_vec = Vec::with_capacity(entries.len());
    let mut total_positions_vec = Vec::with_capacity(entries.len());
    for (key, count_games) in entries {
        material_keys.push(key.to_string());
        counts_games.push(*count_games);
        counts_positions_vec.push(*counts_positions.get(key).unwrap_or(&0));
        total_games_vec.push(total_games);
        total_positions_vec.push(total_positions);
    }

    if let Some(parent) = Path::new(RAW_PARQUET_PATH).parent() {
        fs::create_dir_all(parent)?;
    }

    let mut df = DataFrame::new(vec![
        Series::new("material_key", material_keys),
        Series::new("num_games", counts_games),
        Series::new("num_positions", counts_positions_vec),
        Series::new("total_games", total_games_vec),
        Series::new("total_positions", total_positions_vec),
    ])
    .map_err(polars_to_io_error)?;

    let file = File::create(RAW_PARQUET_PATH)?;
    ParquetWriter::new(file)
        .finish(&mut df)
        .map_err(polars_to_io_error)?;

    Ok(())
}

fn material_key_sizes(df: &DataFrame) -> io::Result<Vec<u64>> {
    let keys = df.column("material_key").map_err(polars_to_io_error)?;
    let keys = keys.str().map_err(polars_to_io_error)?;
    let mut sizes = Vec::with_capacity(keys.len());
    for key in keys.into_iter() {
        let key =
            key.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "material_key is null"))?;
        let material = MaterialKey::from_string(key).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid material_key: {key}"),
            )
        })?;
        sizes.push(PositionIndexer::new(material).total_positions() as u64);
    }
    Ok(sizes)
}

fn polars_to_io_error(err: PolarsError) -> io::Error {
    io::Error::other(err.to_string())
}

struct IndexVisitor<'a> {
    counts_games: &'a mut HashMap<MaterialKey, u64>,
    counts_positions: &'a mut HashMap<MaterialKey, u64>,
    total_positions: &'a mut u64,
    games: u64,
}

struct GameState {
    position: Chess,
    seen: HashSet<MaterialKey>,
}

impl<'a> Visitor for IndexVisitor<'a> {
    type Tags = Option<Chess>;
    type Movetext = GameState;
    type Output = io::Result<()>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(None)
    }

    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        if name == b"FEN" {
            let fen = match Fen::from_ascii(value.as_bytes()) {
                Ok(fen) => fen,
                Err(err) => {
                    return ControlFlow::Break(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid FEN tag: {err}"),
                    )));
                }
            };
            let position = match fen.into_position(CastlingMode::Standard) {
                Ok(pos) => pos,
                Err(err) => {
                    return ControlFlow::Break(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid FEN position: {err}"),
                    )));
                }
            };
            tags.replace(position);
        }
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        let position = tags.unwrap_or_default();
        let mut seen = HashSet::new();
        if let Some(key) = MaterialKey::from_position(&position) {
            *self.total_positions += 1;
            if key.non_pawn_piece_count() <= MAX_NON_PAWN {
                seen.insert(key.clone());
                *self.counts_positions.entry(key).or_insert(0) += 1;
            }
        }
        ControlFlow::Continue(GameState { position, seen })
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn san(
        &mut self,
        movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        let mv = match san_plus.san.to_move(&movetext.position) {
            Ok(mv) => mv,
            Err(err) => {
                return ControlFlow::Break(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("illegal move: {err}"),
                )));
            }
        };
        movetext.position.play_unchecked(mv);
        if let Some(key) = MaterialKey::from_position(&movetext.position) {
            *self.total_positions += 1;
            if key.non_pawn_piece_count() <= MAX_NON_PAWN {
                movetext.seen.insert(key.clone());
                *self.counts_positions.entry(key).or_insert(0) += 1;
            }
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        self.games += 1;
        for key in movetext.seen {
            *self.counts_games.entry(key).or_insert(0) += 1;
        }
        Ok(())
    }
}

fn collect_pgn_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_pgn_files(&path, out)?;
        } else if is_pgn(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_pgn(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pgn") => true,
        Some("gz") => path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.ends_with(".pgn"))
            .unwrap_or(false),
        _ => false,
    }
}

fn is_gz(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("gz")
}

fn is_illegal_move_error(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::InvalidData && err.to_string().starts_with(ILLEGAL_MOVE_PREFIX)
}

fn is_corrupt_gzip_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput
    ) && err.to_string().starts_with(CORRUPT_GZIP_PREFIX)
}

fn is_invalid_fen_tag_error(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::InvalidData && err.to_string().starts_with(INVALID_FEN_TAG_PREFIX)
}

fn is_invalid_fen_position_error(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::InvalidData
        && err.to_string().starts_with(INVALID_FEN_POSITION_PREFIX)
}

#[derive(Default)]
struct SkipStats {
    illegal_moves: u64,
    invalid_fen_tags: u64,
    invalid_fen_positions: u64,
}

impl SkipStats {
    fn report(&self, path: &Path) {
        if self.illegal_moves > 0 {
            eprintln!(
                "Skipped {} games due to illegal moves in {}.",
                self.illegal_moves,
                path.display()
            );
        }
        if self.invalid_fen_tags > 0 {
            eprintln!(
                "Skipped {} games due to invalid FEN tags in {}.",
                self.invalid_fen_tags,
                path.display()
            );
        }
        if self.invalid_fen_positions > 0 {
            eprintln!(
                "Skipped {} games due to invalid FEN positions in {}.",
                self.invalid_fen_positions,
                path.display()
            );
        }
    }
}

fn classify_skip_error(err: &io::Error, stats: &mut SkipStats) -> bool {
    if is_illegal_move_error(err) {
        stats.illegal_moves += 1;
        true
    } else if is_invalid_fen_tag_error(err) {
        stats.invalid_fen_tags += 1;
        true
    } else if is_invalid_fen_position_error(err) {
        stats.invalid_fen_positions += 1;
        true
    } else {
        false
    }
}
