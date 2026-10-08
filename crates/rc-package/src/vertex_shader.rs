//! Bounded diagnostic interpreter for the original hologram vs.1.0 text.
//! Separate f32 operations; no legacy GPU precision guarantee.
use serde::Serialize;
#[derive(Clone, Copy, Debug)]
enum Register {
    State(usize),
    Input(usize),
    Constant(usize),
}
#[derive(Clone, Copy, Debug)]
struct Source {
    register: Register,
    swizzle: [usize; 4],
    negate: bool,
}
#[derive(Clone, Copy, Debug)]
enum Op {
    Mov,
    Add,
    Mul,
    Mad,
    Max,
    Frc,
    Sge,
    Slt,
    Dp3,
    Dp4,
    Rsq,
}
#[derive(Clone, Debug)]
struct Instruction {
    op: Op,
    destination: usize,
    mask: u8,
    sources: Vec<Source>,
}
#[derive(Clone, Debug)]
pub struct Program {
    instructions: Vec<Instruction>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Inputs {
    pub vertices: [[f32; 4]; 16],
    pub constants: Vec<[f32; 4]>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub words: [[u32; 4]; 24],
    pub defined: [[bool; 4]; 24],
}
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub output: [[f32; 4]; 12],
    pub defined: [[bool; 4]; 12],
    pub trace: Vec<Snapshot>,
}
// State: r0..11, oPos, oD0..1, oT0..7, oFog. Output indices subtract 12.
fn register(name: &str) -> Result<Register, String> {
    let number = |prefix: &str, limit: usize| -> Option<usize> {
        let rest = name.strip_prefix(prefix)?;
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        rest.parse::<usize>().ok().filter(|i| *i < limit)
    };
    if name == "oPos" {
        return Ok(Register::State(12));
    }
    if name == "oFog" {
        return Ok(Register::State(23));
    }
    if let Some(i) = number("r", 12) {
        return Ok(Register::State(i));
    }
    if let Some(i) = number("oD", 2) {
        return Ok(Register::State(13 + i));
    }
    if let Some(i) = number("oT", 8) {
        return Ok(Register::State(15 + i));
    }
    if let Some(i) = number("v", 16) {
        return Ok(Register::Input(i));
    }
    let constant = if let Some(n) = name.strip_prefix("c[").and_then(|s| s.strip_suffix(']')) {
        n
    } else {
        name.strip_prefix('c').unwrap_or("")
    };
    if !constant.is_empty() && constant.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(i) = constant.parse::<usize>() {
            if i < 96 {
                return Ok(Register::Constant(i));
            }
        }
    }
    Err(format!("Unsupported vertex register {name}"))
}
fn axes(text: &str) -> Result<Vec<usize>, String> {
    if text.is_empty() || text.len() > 4 {
        return Err("Invalid vertex component selection".into());
    }
    text.chars()
        .map(|c| {
            "xyzw"
                .find(c)
                .ok_or_else(|| "Unsupported vertex component".into())
        })
        .collect()
}
fn uncomment(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut block = false;
    for line in text.lines() {
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if block {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    block = false;
                    out.push(' ');
                }
                continue;
            }
            if c == ';' || c == '/' && chars.peek() == Some(&'/') {
                break;
            }
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                block = true;
                out.push(' ');
                continue;
            }
            out.push(c);
        }
        out.push('\n');
    }
    if block {
        return Err("Unterminated vertex block comment".into());
    }
    Ok(out)
}
impl Program {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 65536 {
            return Err("Vertex shader text exceeds limit".into());
        }
        let text = uncomment(text)?;
        let mut lines = text.lines().map(str::trim).filter(|s| !s.is_empty());
        if !matches!(lines.next(), Some("vs.1.0" | "vs.1.1" | "vs_1_1")) {
            return Err("Expected vertex shader 1.x header".into());
        }
        let mut instructions = vec![];
        for line in lines {
            if instructions.len() >= 128 {
                return Err("Vertex instruction limit".into());
            }
            let (opcode, args) = line
                .split_once(char::is_whitespace)
                .ok_or("Missing vertex operands")?;
            let (op, count) = match opcode {
                "mov" => (Op::Mov, 1),
                "add" => (Op::Add, 2),
                "mul" => (Op::Mul, 2),
                "mad" => (Op::Mad, 3),
                "max" => (Op::Max, 2),
                "frc" => (Op::Frc, 1),
                "sge" => (Op::Sge, 2),
                "slt" => (Op::Slt, 2),
                "dp3" => (Op::Dp3, 2),
                "dp4" => (Op::Dp4, 2),
                "rsq" => (Op::Rsq, 1),
                _ => return Err(format!("Unsupported vertex opcode {opcode}")),
            };
            let args: Vec<_> = args.split(',').map(str::trim).collect();
            if args.len() != count + 1 {
                return Err("Vertex operand count".into());
            }
            let (name, mask) = if let Some((name, selection)) = args[0].split_once('.') {
                let a = axes(selection)?;
                if a.windows(2).any(|v| v[0] >= v[1]) {
                    return Err("Vertex write mask order/duplicate".into());
                }
                (name, a.into_iter().fold(0, |m, i| m | (1 << i)))
            } else {
                (args[0], 15)
            };
            let Register::State(destination) = register(name)? else {
                return Err("Vertex destination must be temporary/output".into());
            };
            if destination == 23 && mask != 1 {
                return Err("Fog output requires .x".into());
            }
            if matches!(op, Op::Frc) && !matches!(mask, 2 | 3) {
                return Err("frc 1.x supports .y/.xy".into());
            }
            let mut sources = vec![];
            for arg in &args[1..] {
                let (negate, arg) = arg.strip_prefix('-').map_or((false, *arg), |s| (true, s));
                let (name, selection) = arg
                    .split_once('.')
                    .map_or((arg, None), |(n, s)| (n, Some(s)));
                let swizzle = if let Some(s) = selection {
                    let a = axes(s)?;
                    std::array::from_fn(|i| a[i.min(a.len() - 1)])
                } else {
                    [0, 1, 2, 3]
                };
                let register = register(name)?;
                if matches!(register, Register::State(12..)) {
                    return Err("Vertex outputs cannot be sources".into());
                }
                if matches!(op, Op::Rsq) && swizzle != [swizzle[0]; 4] {
                    return Err("rsq requires scalar replicate source".into());
                }
                sources.push(Source {
                    register,
                    swizzle,
                    negate,
                });
            }
            instructions.push(Instruction {
                op,
                destination,
                mask,
                sources,
            });
        }
        if instructions.is_empty() {
            return Err("Empty vertex program".into());
        }
        Ok(Self { instructions })
    }
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }
    pub fn evaluate(&self, input: &Inputs) -> Result<Evaluation, String> {
        if input.constants.len() != 96
            || input
                .constants
                .iter()
                .chain(&input.vertices)
                .flatten()
                .any(|v| !v.is_finite())
        {
            return Err("Vertex input constants/finite contract".into());
        }
        let mut values = [[0f32; 4]; 24];
        let mut defined = [[false; 4]; 24];
        let mut trace = vec![];
        for instruction in &self.instructions {
            let read = |source: usize, axis: usize| -> Result<f32, String> {
                let s = instruction.sources[source];
                let a = s.swizzle[axis];
                let v = match s.register {
                    Register::Input(i) => input.vertices[i][a],
                    Register::Constant(i) => input.constants[i][a],
                    Register::State(i) => {
                        if !defined[i][a] {
                            return Err(format!("Undefined vertex temporary r{i} component {a}"));
                        }
                        values[i][a]
                    }
                };
                Ok(if s.negate { -v } else { v })
            };
            let mut result = [0.; 4];
            for (axis, out) in result.iter_mut().enumerate() {
                if instruction.mask & (1 << axis) != 0 {
                    *out = match instruction.op {
                        Op::Mov => read(0, axis)?,
                        Op::Add => read(0, axis)? + read(1, axis)?,
                        Op::Mul => read(0, axis)? * read(1, axis)?,
                        Op::Mad => {
                            let product = read(0, axis)? * read(1, axis)?;
                            product + read(2, axis)?
                        }
                        Op::Max => read(0, axis)?.max(read(1, axis)?),
                        Op::Frc => {
                            let v = read(0, axis)?;
                            v - v.floor()
                        }
                        Op::Sge => {
                            if read(0, axis)? >= read(1, axis)? {
                                1.
                            } else {
                                0.
                            }
                        }
                        Op::Slt => {
                            if read(0, axis)? < read(1, axis)? {
                                1.
                            } else {
                                0.
                            }
                        }
                        Op::Dp3 | Op::Dp4 => {
                            let mut v = read(0, 0)? * read(1, 0)?;
                            for a in 1..if matches!(instruction.op, Op::Dp3) {
                                3
                            } else {
                                4
                            } {
                                v += read(0, a)? * read(1, a)?;
                            }
                            v
                        }
                        Op::Rsq => {
                            let v = read(0, 0)?.abs();
                            if v == 0. {
                                return Err("Zero rsq outside diagnostic finite contract".into());
                            }
                            1. / v.sqrt()
                        }
                    };
                    if !out.is_finite() {
                        return Err("Nonfinite vertex arithmetic".into());
                    }
                }
            }
            for (axis, v) in result.into_iter().enumerate() {
                if instruction.mask & (1 << axis) != 0 {
                    values[instruction.destination][axis] = v;
                    defined[instruction.destination][axis] = true;
                }
            }
            trace.push(Snapshot {
                words: values.map(|v| v.map(f32::to_bits)),
                defined,
            });
        }
        if !defined[12].iter().all(|v| *v) {
            return Err("Incomplete vertex position output".into());
        }
        Ok(Evaluation {
            output: values[12..].try_into().unwrap(),
            defined: defined[12..].try_into().unwrap(),
            trace,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Inputs {
        Inputs {
            vertices: [[0.; 4]; 16],
            constants: vec![[0.; 4]; 96],
        }
    }
    #[test]
    fn comments_swizzle_negation_and_simultaneous_mask_reads() {
        let p = Program::parse(
            "vs.1.0\n/* mov r0,r9 */ mov r0,v0\nadd r0.xy,r0.zx,-r0.yz // test\nmov oPos,r0",
        )
        .unwrap();
        let mut i = input();
        i.vertices[0] = [1., 2., 3., 4.];
        assert_eq!(p.evaluate(&i).unwrap().output[0], [1., -2., 3., 4.]);
    }
    #[test]
    fn fraction_comparison_and_unwritten_output_mask() {
        let p = Program::parse(
            "vs.1.0\nfrc r0.xy,v0\nsge r1.xy,r0,c[0]\nslt r1.zw,v0,c0\nmov oPos,r1\nmov oT0.x,r0.y",
        )
        .unwrap();
        let mut i = input();
        i.vertices[0] = [-0.25, 1.25, -1., 1.];
        i.constants[0] = [0.5; 4];
        let e = p.evaluate(&i).unwrap();
        assert_eq!(e.output[0], [1., 0., 1., 0.]);
        assert_eq!(e.output[3][0], 0.25);
        assert_eq!(e.defined[3], [true, false, false, false]);
    }
    #[test]
    fn dot_rsq_and_mad() {
        let p =
            Program::parse("vs.1.0\ndp3 r0.x,v0,v0\nrsq r0.x,r0.x\nmad oPos,v0,r0.x,c0").unwrap();
        let mut i = input();
        i.vertices[0] = [0., 0., 2., 1.];
        i.constants[0] = [0.5; 4];
        assert_eq!(p.evaluate(&i).unwrap().output[0], [0.5, 0.5, 1.5, 1.]);
    }
    #[test]
    fn malformed_programs_rejected() {
        for source in [
            "vs.2.0\nmov oPos,v0",
            "vs.1.0\nmov r12,v0",
            "vs.1.0\nmov c0,v0",
            "vs.1.0\nmov oPos,c96",
            "vs.1.0\nmov oPos,c[a0.x]",
            "vs.1.0\nmov oPos.xyxy,v0",
            "vs.1.0\nfrc r0.x,v0",
            "vs.1.0\nrsq r0,v0",
            "vs.1.0\n/*",
        ] {
            assert!(Program::parse(source).is_err(), "{source}");
        }
    }
    #[test]
    fn undefined_incomplete_overflow_and_zero_rsq_rejected() {
        for source in [
            "vs.1.0\nmov oPos,r0",
            "vs.1.0\nmov oPos.xyz,v0",
            "vs.1.0\nrsq oPos,v0.x",
        ] {
            assert!(Program::parse(source).unwrap().evaluate(&input()).is_err());
        }
        let mut i = input();
        i.vertices[0] = [f32::MAX; 4];
        assert!(Program::parse("vs.1.0\nmul oPos,v0,v0")
            .unwrap()
            .evaluate(&i)
            .is_err());
    }
}
