use std::path::Path;

use foundation_codegentools::schema_gen::SchemaGenerator;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("schema")
            .about("Generate JSON Schema and Arrow Schema artifacts from derive-annotated structs")
            .arg_required_else_help(true)
            .arg(
                clap::Arg::new("crate")
                    .long("crate")
                    .required(true)
                    .help("Path to the crate directory to scan"),
            )
            .arg(
                clap::Arg::new("jsonschema")
                    .long("jsonschema")
                    .action(clap::ArgAction::SetTrue)
                    .help("Generate JSON Schema files"),
            )
            .arg(
                clap::Arg::new("arrow-schema")
                    .long("arrow-schema")
                    .action(clap::ArgAction::SetTrue)
                    .help("Generate Arrow Schema files"),
            ),
    )
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let crate_dir = args
        .get_one::<String>("crate")
        .expect("--crate is required");
    let crate_path = Path::new(crate_dir);

    let emit_json = args.get_flag("jsonschema");
    let emit_arrow = args.get_flag("arrow-schema");

    if !emit_json && !emit_arrow {
        eprintln!("Error: specify at least one of --jsonschema or --arrow-schema");
        std::process::exit(1);
    }

    let generator = SchemaGenerator::new(crate_path)?;

    println!(
        "Scanning crate: {} ({})",
        generator.crate_name(),
        crate_path.display()
    );
    println!(
        "Found {} structs with schema derives",
        generator.structs().len()
    );

    for s in generator.structs() {
        let derives: Vec<&str> = [
            s.has_arrow_schema.then_some("ArrowSchema"),
            s.has_json_schema.then_some("JsonSchema"),
        ]
        .into_iter()
        .flatten()
        .collect();
        println!(
            "  {} ({} fields, derives: {})",
            s.name,
            s.fields.len(),
            derives.join(", ")
        );
    }

    let generated = generator.generate(emit_json, emit_arrow)?;

    println!();
    println!("Generated {} schema files:", generated.len());
    for g in &generated {
        println!("  {}", g.path.display());
    }

    Ok(())
}
