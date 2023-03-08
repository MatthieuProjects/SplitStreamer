use clap::Parser;
use shared::config::{ClientConfig, VideoBox};

/// A simple program that generates configurations for a given screen combinaison
#[derive(Debug, Parser, Default)]
#[command(author, version, about, long_about = None)]
pub struct ConfGenArgs {
    /// Width of a single screen
    #[arg(long)]
    pub screen_width: usize,
    /// Height of a single screen
    #[arg(long)]
    pub screen_height: usize,

    /// Amount of screens in the x axis
    #[arg(short, long)]
    pub columns: usize,
    /// Amount of screens in the y axis
    #[arg(short, long)]
    pub lines: usize,

    /// Port to use to communicate to the clients
    #[arg(short, long)]
    pub port: u16,
    /// Multicast address to communicate to the clients
    #[arg(short, long)]
    pub address: String,
}

pub fn main() -> anyhow::Result<()> {
    // Get the config
    let screen_config = ConfGenArgs::parse();

    let total_width = screen_config.screen_width * screen_config.columns;
    let total_height = screen_config.screen_height * screen_config.lines;

    let mut screens =
        Vec::<ClientConfig>::with_capacity(screen_config.lines * screen_config.columns);

    for line in 0..screen_config.lines {
        for column in 0..screen_config.columns {
            let top = line * total_height / screen_config.lines;
            let bottom = total_height - (top + total_height / screen_config.lines);

            let left = column * (total_width / screen_config.columns);
            let right = total_width - (left + total_width / screen_config.columns);

            // Now that we have the bounding box of (line, column), we add it to the hashmap
            screens.push(ClientConfig {
                multicast_port: screen_config.port,
                multicast_address: screen_config.address.clone(),
                video_box: VideoBox {
                    top,
                    bottom,
                    left,
                    right,
                },
            })
        }
    }

    println!(
        "{}",
        serde_json::to_string(&screens)?
    );

    Ok(())
}
