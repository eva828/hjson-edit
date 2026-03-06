use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: hjson-edit <command> [args]");
        eprintln!("Commands:");
        eprintln!("  parse <file>     - Parse HJSON file and output AST");
        eprintln!("  format <file>    - Parse and re-output (format preservation test)");
        eprintln!("  edit <file>      - Interactive editor (placeholder)");
        std::process::exit(1);
    }
    
    let command = &args[1];
    
    match command.as_str() {
        "parse" => {
            if args.len() < 3 {
                eprintln!("Usage: hjson-edit parse <file>");
                std::process::exit(1);
            }
            
            let path = &args[2];
            let content = std::fs::read_to_string(path)
                .expect("Failed to read file");
            
            match hjson_edit::parse(&content) {
                Ok(result) => {
                    let red = hjson_edit::RedNode::new_root(result.green);
                    println!("Parsed successfully!");
                    println!("AST: {:?}", red);
                }
                Err(errors) => {
                    eprintln!("Parse errors:");
                    for err in errors {
                        eprintln!("  Line {}, Column {}: {}", err.line, err.column, err.message);
                    }
                    std::process::exit(1);
                }
            }
        }
        
        "format" => {
            if args.len() < 3 {
                eprintln!("Usage: hjson-edit format <file>");
                std::process::exit(1);
            }
            
            let path = &args[2];
            let content = std::fs::read_to_string(path)
                .expect("Failed to read file");
            
            match hjson_edit::parse(&content) {
                Ok(result) => {
                    let output = hjson_edit::green_to_string(&result.green);
                    print!("{}", output);
                }
                Err(errors) => {
                    eprintln!("Parse errors:");
                    for err in errors {
                        eprintln!("  Line {}, Column {}: {}", err.line, err.column, err.message);
                    }
                    std::process::exit(1);
                }
            }
        }
        
        "edit" => {
            println!("Interactive editor mode (placeholder)");
            println!("This would allow editing HJSON while preserving comments and formatting.");
        }
        
        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }
}
