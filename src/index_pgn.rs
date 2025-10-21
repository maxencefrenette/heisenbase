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
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use polars::{
    error::PolarsError,
    prelude::{DataFrame, NamedFrom, ParquetWriter, Series},
};
use shakmaty::{CastlingMode, Chess, Position, fen::Fen};

const PGN_ROOT: &str = "./data/fishtest_pgns";
const TOP_COUNT: usize = 50;
const MAX_NON_PAWN: u32 = 5;
const ILLEGAL_MOVE_PREFIX: &str = "illegal move:";
const INVALID_FEN_TAG_PREFIX: &str = "invalid FEN tag:";
const INVALID_FEN_POSITION_PREFIX: &str = "invalid FEN position:";
const CORRUPT_GZIP_PREFIX: &str = "corrupt gzip stream";
const PARQUET_PATH: &str = "./data/pgn_index.parquet";

pub fn run() -> io::Result<()> {
    let mut files = Vec::new();
    collect_pgn_files(Path::new(PGN_ROOT), &mut files)?;
    files.sort();

    let mut counts: HashMap<MaterialKey, u64> = HashMap::new();
    let mut total_games: u64 = 0;

    for path in files {
        println!("Processing {}", path.display());
        let file = File::open(&path)?;
        let game_count = if is_gz(&path) {
            process_reader(MultiGzDecoder::new(file), &mut counts, &path)?
        } else {
            process_reader(file, &mut counts, &path)?
        };
        total_games += game_count;
    }

    println!("Processed {total_games} games.");

    let mut entries: Vec<_> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    for (idx, (key, count)) in entries.iter().take(TOP_COUNT).enumerate() {
        let percent = if total_games == 0 {
            0.0
        } else {
            (*count as f64 / total_games as f64) * 100.0
        };
        println!(
            "{:>2}. {} ({} games, {:.2}%)",
            idx + 1,
            key,
            *count,
            percent
        );
    }

    write_full_index(&entries)?;

    Ok(())
}

fn process_reader<R: Read>(
    reader: R,
    counts: &mut HashMap<MaterialKey, u64>,
    path: &Path,
) -> io::Result<u64> {
    let mut reader = Reader::new(reader);
    let mut visitor = IndexVisitor { counts, games: 0 };
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

fn write_full_index(entries: &[(MaterialKey, u64)]) -> io::Result<()> {
    let mut material_keys = Vec::with_capacity(entries.len());
    let mut counts = Vec::with_capacity(entries.len());
    for (key, count) in entries {
        material_keys.push(key.to_string());
        counts.push(*count);
    }

    if let Some(parent) = Path::new(PARQUET_PATH).parent() {
        fs::create_dir_all(parent)?;
    }

    let mut df = DataFrame::new(vec![
        Series::new("material_key", material_keys),
        Series::new("num_games", counts),
    ])
    .map_err(polars_to_io_error)?;

    let file = File::create(PARQUET_PATH)?;
    ParquetWriter::new(file)
        .finish(&mut df)
        .map_err(polars_to_io_error)?;

    Ok(())
}

fn polars_to_io_error(err: PolarsError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

struct IndexVisitor<'a> {
    counts: &'a mut HashMap<MaterialKey, u64>,
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
            if key.non_pawn_piece_count() <= MAX_NON_PAWN {
                seen.insert(key);
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
            if key.non_pawn_piece_count() <= MAX_NON_PAWN {
                movetext.seen.insert(key);
            }
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        self.games += 1;
        for key in movetext.seen {
            *self.counts.entry(key).or_insert(0) += 1;
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
