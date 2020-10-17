use super::super::{
    args::BuildArgs,
    manifest::Member,
    srcinfo::database::DatabaseValue,
    status::{Code, Failure, Status},
    utils::{create_makepkg_command, CommandUtils, DbInit, DbInitValue},
};
use command_extra::CommandExtra;
use pipe_trait::*;
use std::{
    fs::copy,
    process::{Command, Stdio},
};

pub fn build(args: BuildArgs) -> Status {
    let BuildArgs {
        syncdeps,
        force,
        pacman,
        log_dest,
        packager,
    } = args;

    let makepkg = || {
        create_makepkg_command()
            .with_arg("--install")
            .with_arg("--noconfirm")
            .with_arg("--asdeps")
            .arg_if("--syncdeps", syncdeps)
            .arg_if("--force", force)
            .may_env("PACMAN", pacman.as_ref())
            .may_env("LOGDEST", log_dest.as_ref())
            .may_env("PACKAGER", packager.as_ref())
    };

    let mut db_init = DbInit::default();
    let DbInitValue {
        database,
        error_count,
        manifest,
    } = db_init.init()?;

    if error_count != 0 {
        eprintln!("{} error occurred", error_count);
        return Code::GenericFailure.into();
    }

    let build_order = match database.build_order() {
        Ok(build_order) => build_order,
        Err(error) => {
            eprintln!("⮾ {}", error);
            return error.code().into();
        }
    };

    let repository = manifest.global_settings.repository.as_path();
    let members: Vec<_> = manifest.resolve_members().collect();

    for pkgbase in build_order {
        let DatabaseValue {
            directory, srcinfo, ..
        } = database.pkgbase().get(pkgbase).unwrap_or_else(|| {
            dbg!(pkgbase);
            panic!("cannot lookup value")
        });

        let Member { directory, .. } = members
            .iter()
            .find(|member| member.directory.as_path() == *directory)
            .unwrap_or_else(|| {
                dbg!(pkgbase, directory);
                panic!("cannot lookup member");
            });

        eprintln!();
        eprintln!();
        eprintln!("==== PACKAGE ====");
        eprintln!();
        eprintln!("🛈 pkgbase:           {}", pkgbase);
        for pkgname in srcinfo.pkgname() {
            eprintln!("🛈 pkgname:           {}", pkgname);
        }
        eprintln!("🛈 source directory:  {}", directory.to_string_lossy());
        eprintln!("🛈 target repository: {}", repository.to_string_lossy());
        eprintln!();

        let repository_directory = repository.parent().expect("get repository directory");
        dbg!(repository_directory);

        #[allow(clippy::all)]
        if !force
            && srcinfo
                .package_file_base_names()
                .expect("get future package file base names")
                .all(|name| {
                    let package_path = repository_directory.join(name.to_string());
                    let exists = package_path.exists();
                    dbg!(&package_path, exists);
                    exists
                })
        {
            eprintln!("🛈 All packages are already built. Skip.");

            let status = pacman
                .as_deref()
                .unwrap_or("pacman")
                .pipe(Command::new)
                .with_arg("--upgrade")
                .with_arg("--noconfirm")
                .with_arg("--asdeps")
                .spawn()
                .and_then(|mut child| child.wait())
                .map_err(|error| {
                    eprintln!("⮾ {}", error);
                    Failure::from(error)
                })?
                .code()
                .unwrap_or(1);
            if status != 0 {
                eprintln!("⮾ pacman -U exits with non-zero status code: {}", status);
            }

            continue;
        }

        let status = makepkg()
            .with_current_dir(directory)
            .with_stdin(Stdio::null())
            .with_stdout(Stdio::inherit())
            .with_stderr(Stdio::inherit())
            .spawn()
            .and_then(|mut child| child.wait())
            .map_err(|error| {
                eprintln!("⮾ {}", error);
                Failure::from(error)
            })?
            .code()
            .unwrap_or(1);

        if status != 0 {
            eprintln!("⮾ makepkg exits with non-zero status code: {}", status);
            return Ok(status);
        }

        for package_name in srcinfo
            .package_file_base_names()
            .expect("get package file base names")
        {
            let package_name = &package_name.to_string();
            eprintln!("📦 made file {}", package_name);
            {
                eprintln!("  → copy to {}/", repository_directory.to_string_lossy());
                if let Err(error) = copy(
                    directory.join(package_name),
                    repository_directory.join(package_name),
                ) {
                    eprintln!("⮾ {}", error);
                    return error.pipe(Failure::from).into();
                }
            }

            {
                eprintln!("  → add to {}", repository.to_string_lossy());
                let status = Command::new("repo-add")
                    .with_arg("--quiet")
                    .with_arg("--nocolor")
                    .with_arg(repository)
                    .with_arg(repository_directory.join(package_name))
                    .with_stdin(Stdio::null())
                    .with_stdout(Stdio::inherit())
                    .with_stderr(Stdio::inherit())
                    .spawn()
                    .and_then(|mut child| child.wait())
                    .map_err(|error| {
                        eprintln!("⮾ {}", error);
                        Failure::from(error)
                    })?
                    .code()
                    .unwrap_or(1);
                if status != 0 {
                    eprintln!("⮾ repo-add exits with non-zero status code: {}", status);
                    return Ok(status);
                }
            }
        }
    }

    Ok(0)
}
