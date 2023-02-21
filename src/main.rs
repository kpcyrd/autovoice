use autovoice::args::Args;
use autovoice::errors::*;
use clap::Parser;
use env_logger::Env;
use futures::prelude::*;
use irc::client::prelude::{ChannelMode, Client, Command, Config, Message, Mode, Prefix, Response};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

pub type Users = HashMap<String, time::Instant>;

// TODO: parse this from ISUPPORT
// we always ignore users with this because they either already have +v or can self-assign it
const SPECIAL_PREFIXES: &[char] = &['@', '+', '~', '&', '%'];

fn has_special_role(nickname: &str) -> bool {
    SPECIAL_PREFIXES.iter().any(|c| nickname.starts_with(*c))
}

async fn random_jitter() {
    time::sleep(Duration::from_millis(fastrand::u64(100..250))).await;
}

fn process_message(users: &mut Users, msg: Message) -> Result<()> {
    let nickname = if let Some(Prefix::Nickname(name, _, _)) = &msg.prefix {
        Some(name)
    } else {
        None
    };

    match (nickname, msg.command) {
        (Some(nickname), Command::JOIN(channel, _, _)) => {
            debug!("User has joined channel (user={nickname:?}, channel={channel:?})");
            users.insert(nickname.to_string(), time::Instant::now());
        }
        (Some(nickname), Command::PART(channel, _)) => {
            debug!("User has disconnected from channel (user={nickname:?}, channel={channel:?}");
            users.remove(nickname);
        }
        (_, Command::Response(Response::RPL_ISUPPORT, data)) => {
            debug!("Received isupport message: {data:?}");
        }
        (_, Command::Response(Response::RPL_NAMREPLY, data)) => {
            let channel = &data.get(2).context("Malformed user-list irc message")?;
            let data = &data.get(3).context("Malformed user-list irc message")?;
            for nickname in data.split(' ') {
                if !has_special_role(nickname) {
                    debug!("User already in channel (user={nickname:?}, channel={channel:?}");
                    users.insert(nickname.to_string(), time::Instant::now());
                }
            }
        }
        _ => (),
    }

    Ok(())
}

fn find_next_promotee(users: &Users, cooldown: Duration) -> Option<String> {
    let now = time::Instant::now();

    for (nickname, join_time) in users {
        if now.duration_since(*join_time) > cooldown {
            return Some(nickname.to_string());
        }
    }

    None
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match args.verbose {
        0 => "warn,autovoice=info",
        1 => "info,autovoice=debug",
        2 => "debug",
        3 => "debug,autovoice=trace",
        _ => "trace",
    };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    let cooldown = if let Some(secs) = args.promote_after_seconds {
        Duration::from_secs(secs)
    } else if let Some(mins) = args.promote_after_minutes {
        Duration::from_secs(mins * 60)
    } else if let Some(hours) = args.promote_after_hours {
        Duration::from_secs(hours * 3600)
    } else {
        bail!("Missing configuration for auto-promotion (eg --promote-after-mins 5)");
    };

    let config = Config {
        nickname: Some(args.nickname.clone()),
        nick_password: args.password,
        server: Some(args.server),
        channels: vec![args.channel.clone()],
        use_tls: Some(true),
        ..Default::default()
    };

    let mut client = Client::from_config(config).await?;

    client
        .identify()
        .context("Failed to identify with irc server")?;

    let mut stream = client.stream().context("Failed to setup irc stream")?;

    let mut users = Users::new();

    let (tx, mut rx) = mpsc::channel::<()>(1);
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if tx.send(()).await.is_err() {
                    break;
                }
            }
        });
    }

    loop {
        tokio::select! {
            msg = stream.next() => {
                if let Some(msg) = msg.transpose().context("Failed to read from irc stream")? {
                    debug!("Received msg from irc server: {msg:?}");
                    if let Err(err) = process_message(&mut users, msg) {
                        error!("Error processing irc message: {err:#}");
                    }
                } else {
                    bail!("irc client has been shutdown");
                }
            }
            _ = rx.recv() => {
                trace!("Checking if any users qualify for promotion");

                if let Some(nickname) = find_next_promotee(&users, cooldown) {
                    if *nickname != args.nickname {
                        info!("Promoting user (user={nickname:?}, channel={:?}", args.channel);
                        let mode = Mode::Plus(ChannelMode::Voice, Some(nickname.to_string()));
                        client.send_mode(&args.channel, &[mode])
                            .context("Failed to set mode for user")?;

                        let tx = tx.clone();
                        tokio::spawn(async move {
                            random_jitter().await;
                            tx.send(()).await.ok();
                        });
                    }
                    users.remove(&nickname);
                }
            }
        }
    }
}
