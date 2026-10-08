//! Bounded ps.1.1 subset for the original hologram fragment program.
//! Diagnostic f32 arithmetic, not old GPU precision/driver equivalence.
use serde::Serialize;
#[derive(Clone, Copy, Debug)]
enum Register {
    R(usize),
    T(usize),
    C(usize),
    V(usize),
}
#[derive(Clone, Copy, Debug)]
struct Source {
    register: Register,
    alpha: bool,
}
#[derive(Clone, Copy, Debug)]
enum Op {
    Mov,
    Mul,
    Dp3,
}
#[derive(Clone, Copy, Debug)]
struct Arithmetic {
    op: Op,
    dest: usize,
    mask: u8,
    saturate: bool,
    a: Source,
    b: Option<Source>,
}
#[derive(Clone, Debug)]
enum Block {
    Tex(usize),
    Arithmetic(Arithmetic),
    Pair(Arithmetic, Arithmetic),
}
#[derive(Clone, Debug)]
pub struct Program {
    blocks: Vec<Block>,
    pub instructions: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Inputs {
    pub textures: [[f32; 4]; 4],
    pub constants: [[f32; 4]; 8],
    pub colors: [[f32; 4]; 2],
}
#[derive(Clone, Debug, Serialize)]
pub struct Registers {
    pub words: [[u32; 4]; 2],
    pub defined: [[bool; 4]; 2],
}
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub color: [f32; 4],
    pub trace: Vec<Registers>,
}
fn register(text: &str) -> Result<Register, String> {
    let b = text.as_bytes();
    if b.len() != 2 || !b[1].is_ascii_digit() {
        return Err(format!("Unsupported register {text}"));
    }
    let i = (b[1] - b'0') as usize;
    match (b[0], i) {
        (b'r', 0..=1) => Ok(Register::R(i)),
        (b't', 0..=3) => Ok(Register::T(i)),
        (b'c', 0..=7) => Ok(Register::C(i)),
        (b'v', 0..=1) => Ok(Register::V(i)),
        _ => Err(format!("Register outside ps.1.1 subset: {text}")),
    }
}
fn source(text: &str) -> Result<Source, String> {
    let (name, alpha) = match text.split_once('.') {
        None => (text, false),
        Some((n, "a")) => (n, true),
        _ => return Err(format!("Unsupported source modifier {text}")),
    };
    Ok(Source {
        register: register(name)?,
        alpha,
    })
}
impl Program {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 65536 {
            return Err("Shader text exceeds limit".into());
        }
        let mut blocks = vec![];
        let mut header = false;
        let mut arithmetic = false;
        let mut instructions = 0;
        let mut loaded = [false; 4];
        for line in text.lines() {
            let line = line
                .split(';')
                .next()
                .unwrap()
                .split("//")
                .next()
                .unwrap()
                .trim();
            if line.is_empty() {
                continue;
            }
            if !header {
                if line != "ps.1.1" && line != "ps_1_1" {
                    return Err("Expected ps.1.1 header".into());
                }
                header = true;
                continue;
            }
            instructions += 1;
            if instructions > 16 {
                return Err("Shader instruction limit".into());
            }
            let (co, line) = if let Some(rest) = line.strip_prefix('+') {
                (true, rest.trim())
            } else {
                (false, line)
            };
            let (opcode, args) = line
                .split_once(char::is_whitespace)
                .ok_or("Missing shader operands")?;
            let args: Vec<_> = args.split(',').map(str::trim).collect();
            if opcode == "tex" {
                if co || arithmetic || args.len() != 1 {
                    return Err("Invalid texture instruction/order".into());
                }
                let Register::T(i) = register(args[0])? else {
                    return Err("tex requires texture register".into());
                };
                if loaded[i] {
                    return Err("Repeated texture load".into());
                }
                loaded[i] = true;
                blocks.push(Block::Tex(i));
                continue;
            }
            arithmetic = true;
            let (opcode, saturate) = opcode
                .strip_suffix("_sat")
                .map_or((opcode, false), |o| (o, true));
            let op = match opcode {
                "mov" => Op::Mov,
                "mul" => Op::Mul,
                "dp3" => Op::Dp3,
                _ => return Err(format!("Unsupported pixel opcode {opcode}")),
            };
            let count = if matches!(op, Op::Mov) { 2 } else { 3 };
            if args.len() != count {
                return Err("Invalid arithmetic operand count".into());
            }
            let (name, mask) = match args[0].split_once('.') {
                None => (args[0], 15),
                Some((n, "rgba")) => (n, 15),
                Some((n, "rgb")) => (n, 7),
                Some((n, "a")) => (n, 8),
                _ => return Err("Unsupported ps.1.1 write mask".into()),
            };
            if matches!(op, Op::Dp3) && mask == 8 {
                return Err("dp3 cannot write only alpha in ps.1.1".into());
            }
            let Register::R(dest) = register(name)? else {
                return Err("Arithmetic requires temporary destination".into());
            };
            let instruction = Arithmetic {
                op,
                dest,
                mask,
                saturate,
                a: source(args[1])?,
                b: if count == 3 {
                    Some(source(args[2])?)
                } else {
                    None
                },
            };
            if co {
                let Some(Block::Arithmetic(previous)) = blocks.pop() else {
                    return Err("Invalid co-issue predecessor".into());
                };
                if previous.mask != 7 || instruction.mask != 8 {
                    return Err("Co-issue requires RGB then alpha".into());
                }
                blocks.push(Block::Pair(previous, instruction));
            } else {
                blocks.push(Block::Arithmetic(instruction));
            }
        }
        if !header || !arithmetic {
            return Err("Empty pixel program".into());
        }
        if blocks
            .iter()
            .filter(|b| !matches!(b, Block::Tex(_)))
            .count()
            > 8
        {
            return Err("ps.1.1 arithmetic slot limit".into());
        }
        Ok(Self {
            blocks,
            instructions,
        })
    }
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
    pub fn evaluate(&self, input: &Inputs) -> Result<Evaluation, String> {
        if input
            .textures
            .iter()
            .chain(&input.constants)
            .chain(&input.colors)
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err("Nonfinite shader input".into());
        }
        let mut r = [[0f32; 4]; 2];
        let mut defined = [[false; 4]; 2];
        let mut loaded = [false; 4];
        let mut trace = vec![];
        let read = |s: Source,
                    axis: usize,
                    r: &[[f32; 4]; 2],
                    defined: &[[bool; 4]; 2],
                    loaded: &[bool; 4]|
         -> Result<f32, String> {
            let axis = if s.alpha { 3 } else { axis };
            Ok(match s.register {
                Register::R(i) => {
                    if !defined[i][axis] {
                        return Err("Uninitialized temporary component".into());
                    }
                    r[i][axis]
                }
                Register::T(i) => {
                    if !loaded[i] {
                        return Err("Texture used before tex".into());
                    }
                    input.textures[i][axis]
                }
                Register::C(i) => input.constants[i][axis],
                Register::V(i) => input.colors[i][axis].clamp(0., 1.),
            })
        };
        for block in &self.blocks {
            if let Block::Tex(i) = block {
                loaded[*i] = true;
            } else {
                let evaluate = |a: Arithmetic| -> Result<[f32; 4], String> {
                    let mut result = [0.; 4];
                    for (axis, out) in result.iter_mut().enumerate() {
                        if a.mask & (1 << axis) != 0 {
                            *out = match a.op {
                                Op::Mov => read(a.a, axis, &r, &defined, &loaded)?,
                                Op::Mul => {
                                    read(a.a, axis, &r, &defined, &loaded)?
                                        * read(a.b.unwrap(), axis, &r, &defined, &loaded)?
                                }
                                Op::Dp3 => {
                                    let b = a.b.unwrap();
                                    let x = read(a.a, 0, &r, &defined, &loaded)?
                                        * read(b, 0, &r, &defined, &loaded)?;
                                    let y = read(a.a, 1, &r, &defined, &loaded)?
                                        * read(b, 1, &r, &defined, &loaded)?;
                                    let z = read(a.a, 2, &r, &defined, &loaded)?
                                        * read(b, 2, &r, &defined, &loaded)?;
                                    (x + y) + z
                                }
                            };
                            if !out.is_finite() {
                                return Err("Pixel arithmetic overflow".into());
                            }
                            if a.saturate {
                                *out = out.clamp(0., 1.);
                            }
                        }
                    }
                    Ok(result)
                };
                let (first, second) = match block {
                    Block::Arithmetic(a) => (*a, None),
                    Block::Pair(a, b) => (*a, Some(*b)),
                    _ => unreachable!(),
                };
                let first_value = evaluate(first)?;
                let second_value = second.map(evaluate).transpose()?;
                for (a, value) in
                    std::iter::once((first, first_value)).chain(second.zip(second_value))
                {
                    for (axis, v) in value.into_iter().enumerate() {
                        if a.mask & (1 << axis) != 0 {
                            r[a.dest][axis] = v;
                            defined[a.dest][axis] = true;
                        }
                    }
                }
            }
            trace.push(Registers {
                words: r.map(|row| row.map(f32::to_bits)),
                defined,
            });
        }
        if !defined[0].into_iter().all(|v| v) {
            return Err("Pixel result r0 is incomplete".into());
        }
        Ok(Evaluation { color: r[0], trace })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn inputs() -> Inputs {
        Inputs {
            textures: [[0.25, 0.5, 1., 0.75]; 4],
            constants: [[1.; 4]; 8],
            colors: [[1.; 4]; 2],
        }
    }
    #[test]
    fn coissue_color_reads_alpha_before_parallel_write() {
        let p = Program::parse("ps.1.1\nmov r1,c0\nmov r0,c0\nmul r0.rgb,r0,r1.a\n+mov r1.a,c1.a")
            .unwrap();
        let mut i = inputs();
        i.constants[0] = [0.5; 4];
        i.constants[1] = [1.; 4];
        let e = p.evaluate(&i).unwrap();
        assert_eq!(e.color, [0.25, 0.25, 0.25, 0.5]);
        assert_eq!(e.trace.last().unwrap().words[1][3], 1f32.to_bits());
    }
    #[test]
    fn coissue_reads_pre_pair_alpha_and_masks_writes() {
        let p = Program::parse(
            "ps.1.1\ntex t0\nmov r0,t0\nmul r0.rgb,r0,c0\n+mov r1.a,r0.a\nmov r0.a,r1.a",
        )
        .unwrap();
        let mut i = inputs();
        i.constants[0] = [0.5; 4];
        assert_eq!(p.evaluate(&i).unwrap().color, [0.125, 0.25, 0.5, 0.75]);
    }
    #[test]
    fn dp3_saturation_replication_and_color_input_clamping() {
        let p = Program::parse("ps.1.1\ntex t0\ndp3_sat r0,t0,c0\nmul r0,r0,v0").unwrap();
        let mut i = inputs();
        i.colors[0] = [-1., 0.5, 2., 1.];
        assert_eq!(p.evaluate(&i).unwrap().color, [0., 0.5, 1., 1.]);
    }
    #[test]
    fn rejects_unknown_syntax_invalid_masks_registers_and_coissue() {
        for source in [
            "ps.2.0\nmov r0,c0",
            "ps.1.1\nadd r0,c0,c1",
            "ps.1.1\nmov r0.x,c0",
            "ps.1.1\nmov r2,c0",
            "ps.1.1\n+mov r0.a,c0.a",
            "ps.1.1\nmov r0,c0\ntex t0",
            "ps.1.1\ndp3 r0.a,c0,c1",
        ] {
            assert!(Program::parse(source).is_err(), "{source}");
        }
    }
    #[test]
    fn undefined_inputs_incomplete_output_and_nonfinite_arithmetic_rejected() {
        assert!(Program::parse("ps.1.1\nmov r0,t0")
            .unwrap()
            .evaluate(&inputs())
            .is_err());
        assert!(Program::parse("ps.1.1\nmov r0.rgb,c0")
            .unwrap()
            .evaluate(&inputs())
            .is_err());
        let mut i = inputs();
        i.constants[0][0] = f32::MAX;
        assert!(Program::parse("ps.1.1\nmul r0,c0,c0")
            .unwrap()
            .evaluate(&i)
            .is_err());
    }
}
