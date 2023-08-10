// Gotham-city
//
// Copyright 2018 by Kzen Networks (kzencorp.com)
// Gotham city is free software: you can redistribute
// it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, help= "Sets the level of verbosity")]
    verbose: bool,

    #[arg(short, long, help= "Address of an electrum address to use. \
    In the format of a sockaddress (url:port)")]
    electrum_address: String,

    #[command(subcommand)]
    command: Commands,

}

#[derive(Subcommand)]
enum Commands {
    /// Create an MPC wallet
    CreateWallet(CreateWalletStruct),

    /// Operation on wallet
    #[command(arg_required_else_help = true)]
    Wallet(WalletStruct)
}

#[derive(Args)]
struct CreateWalletStruct {
    #[arg(short, long, help= "Sets the level of verbosity")]
    verbose: bool,
}

#[derive(Args)]
struct WalletStruct {
    #[arg(short = 'a', long, help= "Generate a new address")]
    new_address: bool,

    #[arg(short = 'b', long, help= "Total balance")]
    get_balance: bool,

    #[arg(short = 'u', long, help= "List unspent transactions (tx hash)")]
    list_unspent: bool,

    #[arg(short = 's', long, help= "Private share backup")]
    backup: bool,

    #[arg(short = 'c', long, help= "Backup verification")]
    verify: bool,

    #[arg(short = 'r', long, help= "Private share recovery")]
    restore: bool,

    #[arg(short = 'o', long, help= "Private shares rotation")]
    rotate: bool,

    #[command(subcommand)]
    command: WalletCommands,
}

#[derive(Subcommand)]
enum WalletCommands {
    /// Send a transaction
    Send(SendStruct),
}

#[derive(Args)]
struct SendStruct {
    #[arg(short, long, help= "Recipient")]
    to: String,

    #[arg(short, long, help= "Amount in BTC")]
    amount: f32
}

fn main() {
    let cli = Cli::parse();

}

/*
#[macro_use]
extern crate clap;
use clap::App;

use client_lib::api;
use client_lib::escrow;
use client_lib::wallet;
use time::PreciseTime;

use std::collections::HashMap;


fn main() {
    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let mut settings = config::Config::default();
    settings
        // Add in `./Settings.toml`
        .merge(config::File::with_name("Settings")).unwrap()
        // Add in settings from the environment (with prefix "APP")
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        .merge(config::Environment::new()).unwrap();
    let hm = settings.try_into::<HashMap<String, String>>().unwrap();
    let endpoint = hm.get("endpoint").unwrap();

    let client_shim = api::ClientShim::new(endpoint.to_string(), None);

    let network = "testnet".to_string();

    if let Some(_matches) = matches.subcommand_matches("create-wallet") {
        println!("Network: [{}], Creating wallet", network);
        let wallet = wallet::Wallet::new(&client_shim, &network);
        wallet.save();
        println!("Network: [{}], Wallet saved to disk", &network);

        let _escrow = escrow::Escrow::new();
        println!("Network: [{}], Escrow initiated", &network);
    } else if let Some(matches) = matches.subcommand_matches("wallet") {
        let mut wallet: wallet::Wallet = wallet::Wallet::load();

        if matches.is_present("new-address") {
            let address = wallet.get_new_bitcoin_address();
            println!("Network: [{}], Address: [{}]", network, address.to_string());
            wallet.save();
        } else if matches.is_present("get-balance") {
            let balance = wallet.get_balance();
            println!(
                "Network: [{}], Balance: [balance: {}, pending: {}]",
                network, balance.confirmed, balance.unconfirmed
            );
        } else if matches.is_present("list-unspent") {
            let unspent = wallet.list_unspent();
            let hashes: Vec<String> = unspent.into_iter().map(|u| u.tx_hash).collect();

            println!(
                "Network: [{}], Unspent tx hashes: [\n{}\n]",
                network,
                hashes.join("\n")
            );
        } else if matches.is_present("backup") {
            let escrow = escrow::Escrow::load();

            println!("Backup private share pending (it can take some time)...");

            let start = PreciseTime::now();
            wallet.backup(escrow);
            let end = PreciseTime::now();

            println!("Backup key saved in escrow (Took: {})", start.to(end));
        } else if matches.is_present("verify") {
            let escrow = escrow::Escrow::load();

            println!("verify encrypted backup (it can take some time)...");

            let start = PreciseTime::now();
            wallet.verify_backup(escrow);
            let end = PreciseTime::now();

            println!(" (Took: {})", start.to(end));
        } else if matches.is_present("restore") {
            let escrow = escrow::Escrow::load();

            println!("backup recovery in process 📲 (it can take some time)...");

            let start = PreciseTime::now();
            wallet::Wallet::recover_and_save_share(escrow, &network, &client_shim);
            let end = PreciseTime::now();

            println!(" Backup recovered 💾(Took: {})", start.to(end));
        } else if matches.is_present("rotate") {
            println!("Rotating secret shares");

            let start = PreciseTime::now();
            let wallet = wallet.rotate(&client_shim);
            wallet.save();
            let end = PreciseTime::now();

            println!("key rotation complete, (Took: {})", start.to(end));
        } else if matches.is_present("send") {
            if let Some(matches) = matches.subcommand_matches("send") {
                let to: &str = matches.value_of("to").unwrap();
                let amount_btc: &str = matches.value_of("amount").unwrap();
                let txid = wallet.send(
                    to.to_string(),
                    amount_btc.to_string().parse::<f32>().unwrap(),
                    &client_shim,
                );
                wallet.save();
                println!(
                    "Network: [{}], Sent {} BTC to address {}. Transaction ID: {}",
                    network, amount_btc, to, txid
                );
            }
        }
    }
}
*/