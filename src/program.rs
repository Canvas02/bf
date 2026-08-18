use std::{fmt::Display, io::Write};

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    Add(usize),
    Sub(usize),
    MoveLeft(usize),
    MoveRight(usize),
    Print(usize),
    Input,
    JumpForward,
    JumpBackward,
}

impl Instruction {
    fn from_bytes(b: &[u8]) -> Vec<Self> {
        b.chunk_by(|a, b| a == b && matches!(a, b'+' | b'-' | b'>' | b'<' | b'.'))
            .filter_map(|chunk| {
                let len = chunk.len();
                match chunk[0] {
                    b'+' => Some(Instruction::Add(len)),
                    b'-' => Some(Instruction::Sub(len)),
                    b'>' => Some(Instruction::MoveRight(len)),
                    b'<' => Some(Instruction::MoveLeft(len)),
                    b'.' => Some(Instruction::Print(len)),
                    b',' => Some(Instruction::Input),
                    b'[' => Some(Instruction::JumpForward),
                    b']' => Some(Instruction::JumpBackward),
                    _ => None,
                }
            })
            .collect()
    }
}

pub struct Program {
    program: Vec<Instruction>,
    jump_map: Vec<usize>,
    stack: [u8; 1024 * 60],
    sp: usize, // stack pinter

    pc: usize, // program counter
}

impl Program {
    pub fn new(program_source: &[u8]) -> ProgramResult<Self> {
        let program = Instruction::from_bytes(program_source);

        let mut jump_map = vec![0usize; program.len()];
        let mut start_stack: Vec<usize> = vec![];

        for (i, inst) in program.iter().enumerate() {
            match inst {
                Instruction::JumpForward => start_stack.push(i),
                Instruction::JumpBackward => {
                    let start = start_stack
                        .pop()
                        .ok_or(ProgramError::BadLoopStackReference)?;
                    jump_map[start] = i;
                    jump_map[i] = start;
                }
                _ => {}
            }
        }

        if !start_stack.is_empty() {
            return Err(ProgramError::BadLoopStackReference);
        }

        Ok(Self {
            program,
            jump_map,
            stack: [0u8; _],
            sp: 0,

            pc: 0, // program counter
        })
    }

    pub fn interpret(mut self) -> ProgramResult<()> {
        while self.pc < self.program.len() {
            match self.program[self.pc] {
                Instruction::Add(x) => {
                    self.stack[self.sp] = self.stack[self.sp].wrapping_add(x as u8)
                }
                Instruction::Sub(x) => {
                    self.stack[self.sp] = self.stack[self.sp].wrapping_sub(x as u8)
                }

                Instruction::MoveRight(x) => {
                    self.sp = self
                        .sp
                        .checked_add(x)
                        .filter(|&new_sp| new_sp < self.stack.len())
                        .ok_or(ProgramError::TapePointerOutOfBounds)?;
                }
                Instruction::MoveLeft(x) => {
                    self.sp = self
                        .sp
                        .checked_sub(x)
                        .ok_or(ProgramError::TapePointerOutOfBounds)?;
                }

                Instruction::Print(x) => {
                    for _ in 0..x {
                        if self.stack[self.sp] >= 32 && self.stack[self.sp] <= 126 {
                            print!("{}", self.stack[self.sp] as char)
                        } else {
                            print!("{}", self.stack[self.sp])
                        }
                    }
                }
                Instruction::Input => {
                    print!("> ");
                    std::io::stdout().flush()?;
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf)?;
                    println!();

                    let buf = buf.trim(); // Remove trailing new line

                    self.stack[self.sp] =
                        if !buf.is_empty() && buf.chars().all(|c| char::is_ascii_digit(&c)) {
                            buf.parse::<u8>()?
                        } else {
                            // If empty throws error, else gets first char
                            *buf.as_bytes().first().ok_or(ProgramError::EmptyInput)?
                        }
                }
                Instruction::JumpForward => {
                    if self.stack[self.sp] == 0 {
                        self.pc = self.jump_map[self.pc];
                    }
                }
                Instruction::JumpBackward => {
                    if self.stack[self.sp] != 0 {
                        self.pc = self.jump_map[self.pc];
                    }
                }
            }
            self.pc += 1;
        }

        println!();
        Ok(())
    }

    // ----- C Transpiler
    fn prelude() -> &'static str {
        r#"// Prelude-begin
        #include <assert.h>
        #include <stdio.h>
        #include <stdint.h>
        #include <stdlib.h>
        #include <string.h>
        #include <ctype.h>

        #define STACK_SIZE 60

        uint8_t read_input(void) {
            printf("> ");
            fflush(stdout);

            char buf[256];
            if (fgets(buf, sizeof(buf), stdin) == NULL) {
                fprintf(stderr, "No input provided.\n");
                exit(1);
            }

            // trim new lines
            size_t len = strlen(buf);
            while (len > 0 && (buf[len - 1] == '\n' || buf[len - 1] == '\r')) {
                buf[--len] = '\0';
            }

            if (len == 0) {
                fprintf(stderr, "Empty input.\n");
                exit(1);
            }

            uint8_t all_digits = 1;
            for (size_t i = 0; i < len; i++) {
                if (!isdigit((unsigned char)buf[i])) {
                    all_digits = 0;
                    break;
                }
            }

            if (all_digits) {
                long val = strtol(buf, NULL, 10);
                if (val < 0 || val > 255) {
                    fprintf(stderr, "Input out of u8 range.\n");
                    exit(1);
                } else {
                    return (uint8_t)val;
                }
            } else {
                return (uint8_t)buf[0];
            }
        }

        void checked_add_size_t(size_t a, size_t b, size_t* out) {
            size_t result = a + b;
            if (result < a)
                assert(0 && "SP overflow");

            *out = result;
        }

        void checked_sub_size_t(size_t a, size_t b, size_t* out) {
            if (b > a)
                assert(0 && "SP underflow");

            *out = a - b;
        }

        void bf_print(uint8_t x) {
            if (x >= 32 && x <= 126) {
                printf("%c", x);
            } else {
                printf("%d", x);
            }
        }

        int main() {
            uint8_t stack[1024 * STACK_SIZE] = {0};
            size_t sp = 0;

        // Prelude-end
        "#
    }

    fn epilogue() -> &'static str {
        r#"
            // Epilogue-start
            printf("\n");
            return 0;
        }
        // Epilogue-end
        "#
        .trim()
    }

    pub fn compiler(&self) -> String {
        let mut s = "".to_owned();
        s.push_str(Self::prelude());
        let mut pc = self.pc;
        while pc < self.program.len() {
            match self.program[pc] {
                Instruction::Add(x) => s += &format!("stack[sp] += {};\n", x),
                Instruction::Sub(x) => s += &format!("stack[sp] -= {};\n", x),

                Instruction::MoveRight(x) => s += &format!("checked_add_size_t(sp, {}, &sp);\n", x),
                Instruction::MoveLeft(x) => s += &format!("checked_sub_size_t(sp, {}, &sp);\n", x),

                Instruction::Print(x) => {
                    s += &format!("for (int i = 0; i < {}; i++)  bf_print(stack[sp]);\n", x)
                }
                Instruction::Input => s += "stack[sp] = read_input();\n",

                Instruction::JumpForward => s += "while (stack[sp]) {\n",
                Instruction::JumpBackward => s += "}",
            }
            // s += "pc++;";
            pc += 1;
        }

        s.push_str(Self::epilogue());
        s
    }
}

pub type ProgramResult<T> = Result<T, ProgramError>;

#[derive(Debug)]
pub enum ProgramError {
    InputParseError(std::num::ParseIntError),
    BadLoopStackReference,
    StdioError(std::io::Error),
    EmptyInput,
    TapePointerOutOfBounds,
}

impl Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::InputParseError(e) => write!(f, "Unable to parse input as u8: {}", e),
            Self::BadLoopStackReference => write!(f, "No loop entries in loop stack"),
            Self::StdioError(e) => write!(f, "Failure in stdio: {}", e),
            Self::EmptyInput => write!(f, "No input given when requested"),
            Self::TapePointerOutOfBounds => write!(f, "Tape moved out of bounds"),
        }
    }
}

impl std::error::Error for ProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InputParseError(e) => Some(e),
            Self::StdioError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProgramError {
    fn from(err: std::io::Error) -> Self {
        ProgramError::StdioError(err)
    }
}

impl From<std::num::ParseIntError> for ProgramError {
    fn from(err: std::num::ParseIntError) -> Self {
        ProgramError::InputParseError(err)
    }
}
