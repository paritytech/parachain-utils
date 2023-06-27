use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Tool to add sudo-pallet to relay chains in the polkadot repo
fn main() {
    if std::env::args().count() != 1 {
        println!("Usage: No args, just run in a polkadot repo dir to add sudo pallet");
        std::process::exit(1);
    }

    let workspace: PathBuf = std::env::current_dir().expect("can't get current dir");
    add_sudo(&workspace, "kusama");
    add_sudo(&workspace, "polkadot");
}

fn add_sudo(workspace: &Path, runtime_name: &str) {
    let chainspec_rs = workspace.join("node/service/src/chain_spec.rs");
    let runtime_dir = workspace.join("runtime").join(runtime_name);

    let cargo_toml = runtime_dir.join("Cargo.toml");

    let lib_rs = runtime_dir.join("src").join("lib.rs");
    let mut cargo_contents = read_to_string(&cargo_toml);
    let branch = sniff_branch(&cargo_contents).expect("can't sniff branch");
    let sudo_crate = format!(
        r#"pallet-sudo = {{ git = "https://github.com/paritytech/substrate", default-features = false, branch = "{}" }}"#,
        branch
    );
    let mut lib_contents = read_to_string(&lib_rs);

    if !cargo_contents.contains(&sudo_crate) {
        cargo_contents = cargo_contents.replace(
            "[dev-dependencies]",
            &(sudo_crate + "\n\n[dev-dependencies]"),
        );
        write(&cargo_toml, &cargo_contents);
    }

    if !cargo_contents.contains("pallet-sudo/std") {
        cargo_contents = cargo_contents.replace(
            "\"pallet-staking/std\",",
            "\"pallet-staking/std\",\n\t\"pallet-sudo/std\",",
        );
        write(&cargo_toml, &cargo_contents);
    }

    if !lib_contents.contains("Sudo: pallet_sudo") {
        if let Some(index) = lib_contents
            .lines()
            .position(|line| line.contains("construct_runtime! {"))
        {
            let pos = lib_contents
                .lines()
                .skip(index)
                .position(|line| line.ends_with("\t}"));
            if let Some(pos) = pos {
                let mut lines = lib_contents.lines().collect::<Vec<_>>();
                lines.insert(pos + index, "\t\tSudo: pallet_sudo = 255,");
                lib_contents = lines.join("\n");
                write(&lib_rs, &lib_contents);
            }
        }
    }

    if !lib_contents.contains("impl pallet_sudo::Config for Runtime") {
        lib_contents.push_str(
            "

impl pallet_sudo::Config for Runtime {
\ttype RuntimeEvent = RuntimeEvent;
\ttype RuntimeCall = RuntimeCall;
\ttype WeightInfo = ();
}
",
        );
        // \ttype WeightInfo = (); with the new version
        write(lib_rs, lib_contents);
    }

    // Now let's add in the genesis config
    let mut chain_spec_contents = read_to_string(&chainspec_rs);
    if !chain_spec_contents.contains(&format!("sudo: {}::SudoConfig", runtime_name)) {
        chain_spec_contents = chain_spec_contents.replace(
            &format!("\t{}::RuntimeGenesisConfig {{", runtime_name),
            &format!(
                "\t{}::RuntimeGenesisConfig {{
\t\tsudo: {}::SudoConfig {{
\t\t\tkey: Some(get_account_id_from_seed::<sr25519::Public>(\"Alice\")),
\t\t}},",
                runtime_name, runtime_name
            ),
        );
        write(&chainspec_rs, &chain_spec_contents);
    }
    if !chain_spec_contents.contains(&format!("sudo: {}::SudoConfig", runtime_name)) {
        chain_spec_contents = chain_spec_contents.replace(
            &format!("\t{}::GenesisConfig {{", runtime_name),
            &format!(
                "\t{}::GenesisConfig {{
\t\tsudo: {}::SudoConfig {{
\t\t\tkey: Some(get_account_id_from_seed::<sr25519::Public>(\"Alice\")),
\t\t}},",
                runtime_name, runtime_name
            ),
        );
        write(&chainspec_rs, &chain_spec_contents);
    }

    std::process::Command::new("cargo")
        .args(["metadata"])
        .current_dir(workspace)
        .output()
        .expect("please build the workspace to update the Cargo.lock yourself.");
}

pub fn read_to_string<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("can't read {}", path.as_ref().display()))
}

fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) {
    fs::write(&path, contents)
        .unwrap_or_else(|_| panic!("can't write to {}", path.as_ref().display()));
}

fn sniff_branch(cargo_toml: &str) -> Option<&str> {
    let line = cargo_toml
        .lines()
        .find(|line| line.contains("git = \"https://github.com/paritytech/substrate\""))?;
    let branch_patern = "branch = \"";
    let pos = line.find(branch_patern)? + branch_patern.len();
    line[pos..].split('"').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniff_branch() {
        let toml = r#"sp-io = { git = "https://github.com/paritytech/substrate", branch = "polkadot-v0.9.38" }"#;
        assert_eq!(sniff_branch(toml), Some("polkadot-v0.9.38"));
    }
}
