use crate::program::Program;

mod program;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let flags: Vec<String> = args.extract_if(.., |s| s.starts_with("--")).collect();
    if args.len() != 2 {
        eprintln!("No argument given/Too many arguments");
        eprintln!("{} (--compiled) (--opt) source_file", &args[0]);
        return;
    }

    let program_source = std::fs::read_to_string(&args[1]).unwrap();
    let program = Program::new(program_source.as_bytes());

    let source_file_name_raw = args[1].to_string();
    let source_file_name = std::path::Path::new(&source_file_name_raw)
        .file_stem()
        .unwrap()
        .to_string_lossy();

    let opt_flag = if flags.iter().any(|s| s == "--opt") {
        "-O2"
    } else {
        ""
    };

    if flags.iter().any(|s| s == "--compiled") {
        std::fs::create_dir_all("build").expect("Failed to create directory: build");
        std::fs::write(format!("build/{}.c", source_file_name), program.compiler()).expect(
            &format!("Failed to write file: build/{}.c", source_file_name),
        );

        let cc = std::process::Command::new("cc")
            .arg("-o")
            .arg(&format!("build/{}", source_file_name))
            .arg(&format!("build/{}.c", source_file_name))
            .arg(opt_flag)
            .output()
            .expect("Failed to run cc");

        println!("Compiled with cc: {}", cc.status);
    } else {
        program.interpret().unwrap();
    }
}
