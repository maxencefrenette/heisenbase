/// Contains the code to index PGN files to find the most common material keys.
/// Stores the results in sqlite tables.
use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::File,
    io,
    io::Read,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use flate2::read::MultiGzDecoder;
use heisenbase::material_key::MaterialKey;
use heisenbase::position_indexer::PositionIndexer;
use heisenbase::storage;
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use rusqlite::params;
use shakmaty::{CastlingMode, Chess, Position, fen::Fen};

const PGN_ROOT: &str = "./data/fishtest_pgns";
const MAX_NON_PAWN: u32 = 5;
const ILLEGAL_MOVE_PREFIX: &str = "illegal move:";
const INVALID_FEN_TAG_PREFIX: &str = "invalid FEN tag:";
const INVALID_FEN_POSITION_PREFIX: &str = "invalid FEN position:";
const CORRUPT_GZIP_PREFIX: &str = "corrupt gzip stream";

pub fn run_stage1() -> Result<()> {
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

pub fn run_stage2() -> Result<()> {
    let mut conn = storage::open_database()?;
    let entries = {
        let mut stmt = conn.prepare(
            "SELECT material_key, num_games, num_positions, total_games, total_positions
             FROM pgn_index_raw
             WHERE num_games > 1",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        entries
    };

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM pgn_index", [])?;
    let mut insert = tx.prepare(
        "INSERT INTO pgn_index (
            material_key,
            num_games,
            num_positions,
            total_games,
            total_positions,
            material_key_size,
            num_pieces,
            num_pawns,
            num_non_pawns,
            utility
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    for (material_key, num_games, num_positions, total_games, total_positions) in entries {
        let material = parse_material_key(&material_key)?;
        let material_key_size = PositionIndexer::new(material.clone()).total_positions() as i64;
        let num_pieces = material.total_piece_count() as i64;
        let num_pawns = material.pawns.pawn_count() as i64;
        let num_non_pawns = material.non_pawn_piece_count() as i64;
        // Utility is the share of all indexed PGN positions occupied by this material key.
        // We do not divide by material_key_size here. That normalization is applied later
        // when ranking candidates so direct and transitive utility live in the same units.
        let utility = if total_positions > 0 {
            num_positions as f64 / total_positions as f64
        } else {
            0.0
        };
        insert.execute(params![
            material_key,
            num_games,
            num_positions,
            total_games,
            total_positions,
            material_key_size,
            num_pieces,
            num_pawns,
            num_non_pawns,
            utility,
        ])?;
    }
    drop(insert);
    tx.commit()?;

    Ok(())
}

fn process_reader<R: Read>(
    reader: R,
    counts_games: &mut HashMap<MaterialKey, u64>,
    counts_positions: &mut HashMap<MaterialKey, u64>,
    total_positions: &mut u64,
    path: &Path,
) -> Result<u64> {
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
            Err(err) => return Err(err.into()),
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
) -> Result<()> {
    let mut conn = storage::open_database()?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM pgn_index_raw", [])?;
    let mut insert = tx.prepare(
        "INSERT INTO pgn_index_raw (
            material_key,
            num_games,
            num_positions,
            total_games,
            total_positions
        ) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for (key, count_games) in entries {
        insert.execute(params![
            key.to_string(),
            *count_games as i64,
            *counts_positions.get(key).unwrap_or(&0) as i64,
            total_games as i64,
            total_positions as i64,
        ])?;
    }

    drop(insert);
    tx.commit()?;
    Ok(())
}

fn parse_material_key(key: &str) -> Result<MaterialKey> {
    MaterialKey::from_string(key).map_err(|err| anyhow!("invalid material_key: {key}: {err}"))
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
    type Output = Result<()>;

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
                    )
                    .into()));
                }
            };
            let position = match fen.into_position(CastlingMode::Standard) {
                Ok(pos) => pos,
                Err(err) => {
                    return ControlFlow::Break(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid FEN position: {err}"),
                    )
                    .into()));
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
                )
                .into()));
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

fn collect_pgn_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
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

fn classify_skip_error(err: &anyhow::Error, stats: &mut SkipStats) -> bool {
    let Some(io_err) = extract_io_error(err) else {
        return false;
    };

    if is_illegal_move_error(io_err) {
        stats.illegal_moves += 1;
        true
    } else if is_invalid_fen_tag_error(io_err) {
        stats.invalid_fen_tags += 1;
        true
    } else if is_invalid_fen_position_error(io_err) {
        stats.invalid_fen_positions += 1;
        true
    } else {
        false
    }
}

fn extract_io_error(err: &anyhow::Error) -> Option<&io::Error> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
}
