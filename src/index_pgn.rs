use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::File,
    io,
    io::Read,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use heisenbase::material_key::MaterialKey;
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{CastlingMode, Chess, Position, fen::Fen};

const PGN_ROOT: &str = "./data/fishtest_pgns";
const TOP_COUNT: usize = 50;

pub fn run() -> io::Result<()> {
    let mut files = Vec::new();
    collect_pgn_files(Path::new(PGN_ROOT), &mut files)?;
    files.sort();

    let mut counts: HashMap<MaterialKey, u64> = HashMap::new();
    for path in files {
        if is_gz(&path) {
            let file = File::open(&path)?;
            process_reader(GzDecoder::new(file), &mut counts)?;
        } else {
            let file = File::open(&path)?;
            process_reader(file, &mut counts)?;
        }
    }

    let mut entries: Vec<_> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    for (idx, (key, count)) in entries.into_iter().take(TOP_COUNT).enumerate() {
        println!("{:>2}. {} ({})", idx + 1, key, count);
    }

    Ok(())
}

fn process_reader<R: Read>(reader: R, counts: &mut HashMap<MaterialKey, u64>) -> io::Result<()> {
    let mut reader = Reader::new(reader);
    let mut visitor = IndexVisitor { counts };
    while let Some(result) = reader.read_game(&mut visitor)? {
        result?;
    }
    Ok(())
}

struct IndexVisitor<'a> {
    counts: &'a mut HashMap<MaterialKey, u64>,
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
            seen.insert(key);
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
            movetext.seen.insert(key);
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
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
