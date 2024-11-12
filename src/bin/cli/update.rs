use ahitool::tools::update::{update_executable, GITHUB_REPO};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The GitHub repository to check for updates.
    #[arg(long, default_value = GITHUB_REPO)]
    repo: String,
}

pub fn main(args: Args) -> anyhow::Result<()> {
    let Args { repo } = args;
    update_executable(&repo)
}
