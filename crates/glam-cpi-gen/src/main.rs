use anchor_idl::GeneratorOptions;
use clap::{Parser, Subcommand};
use prettyplease::unparse;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, File};

#[derive(Parser)]
#[command(name = "glam-cpi-gen")]
#[command(about = "Generates CPI interface from an IDL file", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate CPI interface for external programs
    Cpi {
        #[arg(required = true, help = "Path to the IDL file")]
        idl_path: String,

        #[arg(short, long, required = true, help = "Program ID")]
        program_id: String,

        #[arg(long, help = "Output file path for the generated code")]
        output: Option<String>,
    },
    /// Generate GLAM CPI wrapper implementation
    Glam {
        #[arg(required = true, help = "Path to the IDL file")]
        idl_path: String,

        #[arg(short, long, help = "IDL name alias")]
        idl_name_alias: Option<String>,

        #[arg(short, long, action = clap::ArgAction::SetTrue, help = "Skip generating imports")]
        skip_imports: bool,

        #[arg(
            short,
            long,
            help = "Output file path for the generated CPI proxy code"
        )]
        output: Option<String>,

        #[arg(short, long, help = "Configuration file path")]
        config: Option<String>,

        #[arg(
            short = 'I',
            long,
            help = "Instructions to generate CPI for (generate all if not specified)"
        )]
        ixs: Option<Vec<String>>,
    },
    /// Generate instruction remapping information
    Remapping {
        #[arg(required = true, help = "Path to the IDL file")]
        idl_path: String,

        #[arg(short, long, help = "IDL name alias")]
        idl_name_alias: Option<String>,

        #[arg(short, long, required = true, help = "Program ID")]
        program_id: String,

        #[arg(
            short,
            long,
            help = "Output JSON file containing ix remapping information"
        )]
        output: Option<String>,

        #[arg(short, long, help = "Configuration file path")]
        config: Option<String>,

        #[arg(
            short = 'I',
            long,
            help = "Instructions to generate remapping for (generate all if not specified)"
        )]
        ixs: Option<Vec<String>>,
    },
}

fn prettify(tokens: TokenStream) -> String {
    let syntax_tree: File = parse2(tokens).expect("Failed to parse TokenStream");
    let pretty_code = unparse(&syntax_tree);

    pretty_code
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cpi {
            idl_path,
            program_id,
            output,
        } => {
            let opts = GeneratorOptions {
                idl_path,
                ..Default::default()
            };
            let generator = opts.to_generator();

            let mut token_stream = TokenStream::new();
            token_stream.extend(quote! {
                use anchor_lang::declare_id;
                declare_id!(#program_id);
            });
            token_stream.extend(generator.generate_cpi_interface());
            let pretty_code = prettify(token_stream);

            if let Some(output_file) = output {
                std::fs::write(output_file, pretty_code).unwrap();
            } else {
                println!("{}", pretty_code);
            }
        }
        Commands::Glam {
            idl_path,
            idl_name_alias,
            skip_imports,
            output,
            config,
            ixs,
        } => {
            let opts = GeneratorOptions {
                idl_path,
                idl_name_alias: idl_name_alias.clone(),
                glam_codegen_config: config,
                ..Default::default()
            };
            let generator = opts.to_generator();

            let (glam_code, _) = generator.generate_glam_code(
                &ixs.unwrap_or_default(),
                skip_imports,
                idl_name_alias,
            );
            let pretty_code = prettify(glam_code);

            if let Some(output_file) = output {
                std::fs::write(output_file, pretty_code).unwrap();
            } else {
                print!("{}", pretty_code);
            }
        }
        Commands::Remapping {
            idl_path,
            idl_name_alias,
            program_id,
            output,
            config,
            ixs,
        } => {
            let opts = GeneratorOptions {
                idl_path,
                idl_name_alias: idl_name_alias.clone(),
                glam_codegen_config: config,
                ..Default::default()
            };
            let generator = opts.to_generator();

            let (_, mut ixs_remapping) = generator.generate_glam_code(
                &ixs.unwrap_or_default(),
                true, // skip_imports is always true for remapping
                idl_name_alias,
            );
            ixs_remapping.program_id = program_id;

            let content = serde_json::to_string_pretty(&ixs_remapping).unwrap();
            if let Some(output) = output {
                std::fs::write(output, content).unwrap();
            } else {
                print!("{}", content);
            }
        }
    }
}
