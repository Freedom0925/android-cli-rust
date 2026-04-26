//! Android CLI entry point

use anyhow::Result;
use clap::Parser;

mod cli;

use cli::{Cli, Commands, Context, execute_sdk, execute_emulator, execute_device,
          execute_run, execute_skills, execute_template, execute_init, execute_describe,
          execute_docs, execute_update, execute_info, execute_create, execute_screen,
          execute_layout, execute_help, execute_upload_metrics, execute_test_metrics};

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();

    let ctx = Context::new(cli.sdk, cli.sdk_index, cli.sdk_url, cli.no_metrics)?;

    match cli.command {
        Commands::Sdk { command } => execute_sdk(command, &ctx),
        Commands::Emulator { command } => execute_emulator(command, &ctx),
        Commands::Device { command } => execute_device(command, &ctx),
        Commands::Run {
            apks,
            device,
            type_,
            activity,
            debug,
        } => execute_run(&apks, &device, &type_, &activity, debug, &ctx),
        Commands::Skills { command } => execute_skills(command, &ctx),
        Commands::Template {
            process,
            profile,
            output,
        } => execute_template(&process, &profile, &output, &ctx),
        Commands::Init { force } => execute_init(force, &ctx),
        Commands::Describe { project_dir } => execute_describe(project_dir.as_deref(), &ctx),
        Commands::Docs { command } => execute_docs(command),
        Commands::Update { url } => execute_update(url.as_deref()),
        Commands::Info { field } => execute_info(field.as_deref(), &ctx),
        Commands::Create {
            name,
            output,
            min_sdk,
            list,
            template,
            verbose,
            dry_run: _,
        } => execute_create(
            name.as_deref(),
            &output,
            min_sdk.as_deref(),
            list,
            template.as_deref(),
            verbose,
            &ctx,
        ),
        Commands::Screen { command } => execute_screen(command, &ctx),
        Commands::Layout {
            output,
            diff,
            pretty,
            device,
        } => execute_layout(output.as_deref(), diff, pretty, device.as_deref(), &ctx),
        Commands::Help { command } => execute_help(command.as_deref()),
        Commands::UploadMetrics => execute_upload_metrics(&ctx),
        Commands::TestMetrics { command } => execute_test_metrics(command),
    }?;

    Ok(())
}