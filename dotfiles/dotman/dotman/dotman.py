import os
import typer
import toml
from pathlib import Path

dotman_cli = typer.Typer()


class DotmanConfig:
    def __init__(self, dotman_config_file_directory, target_config_directory, file_map):
        self.source_dir = dotman_config_file_directory
        self.target_dir = target_config_directory
        self.file_map = file_map


def main():
    dotman_cli()


def load_dotman_configurations(root_path: Path) -> list[DotmanConfig]:
    if not root_path.is_dir():
        typer.echo(f"Error: {root_path} is not a directory!")
        return []

    (_, root_path_subdirectories, _) = next(os.walk(root_path))
    root_path_subdirectories = [root_path.joinpath(subdir) for subdir in root_path_subdirectories]

    dotman_config_file_directories = []
    for subdir in root_path_subdirectories:
        (_, _, config_files) = next(os.walk(subdir))
        config_files = [x for x in config_files if x == "dotman.toml"]
        if len(config_files) > 0:
            metadata_file = config_files[0]
            typer.echo(f"Found {metadata_file} at {subdir}.")
            dotman_config_file_directories.append(Path(subdir))

    dotman_configurations = []
    for source_dir in dotman_config_file_directories:
        dotman_dir = source_dir.joinpath("dotman.toml")
        metadata = toml.load(dotman_dir)
        entries = metadata.get("dotman")
        if entries is None:
            typer.echo(f"Error: Missing [dotman] on {dotman_dir}.")
            continue

        config_path = entries.get("config_path")
        if config_path is None:
            typer.echo(f"Error: Missing config_path on [dotman] on {dotman_dir}.")
            continue

        config_path = Path(os.path.expandvars(config_path)).expanduser()
        if not config_path.is_absolute():
            typer.echo(f"Error: config_path: '{config_path}' must be an absolute path.")
            continue

        file_map = entries.get("file_map") or []
        dotman_configurations.append(DotmanConfig(source_dir, config_path, file_map))

    return dotman_configurations


@dotman_cli.command()
def link(root_path: Path):
    dotman_configurations = load_dotman_configurations(root_path)

    for dotman_config in dotman_configurations:
        source_dir = dotman_config.source_dir
        target_dir = dotman_config.target_dir
        file_map = dotman_config.file_map

        if not os.path.exists(target_dir):
            typer.echo(f"{source_dir}: Creating {target_dir}.")
            os.makedirs(target_dir)

        # Support both regular files and directories (excluding dotman.toml)
        source_entries = [p for p in source_dir.iterdir() if p.name != "dotman.toml"]
        for source_entry in source_entries:
            source_file_path = os.path.realpath(source_entry)
            config_name = source_entry.name

            target_file_path = target_dir.joinpath(config_name)
            for mapping in file_map:
                if mapping["file"] == config_name:
                    target_file_path = target_dir.joinpath(Path(mapping["link"]))

            if os.path.exists(source_file_path) or os.path.islink(source_file_path):
                if os.path.islink(target_file_path):
                    typer.echo(f"{source_dir}: Removing existant link at {target_file_path}.")
                    os.unlink(target_file_path)
                elif os.path.isfile(target_file_path):
                    typer.echo(f"{source_dir}: Removing existant file at {target_file_path}.")
                    os.remove(target_file_path)
                elif os.path.isdir(target_file_path):
                    typer.echo(f"{source_dir}: Target {target_file_path} is an existing directory; cannot replace directly.")
                    continue

                typer.echo(f"{source_dir}: Creating link: {target_file_path} -> {source_file_path}.")
                os.symlink(source_file_path, target_file_path)
            else:
                typer.echo(f"{source_file_path} does not exist")


@dotman_cli.command()
def unlink(root_path: Path):
    dotman_configurations = load_dotman_configurations(root_path)

    for dotman_config in dotman_configurations:
        source_dir = dotman_config.source_dir
        target_dir = dotman_config.target_dir
        file_map = dotman_config.file_map

        if not target_dir.exists():
            continue

        source_entries = [p for p in source_dir.iterdir() if p.name != "dotman.toml"]
        for source_entry in source_entries:
            source_file_path = os.path.realpath(source_entry)
            config_name = source_entry.name

            target_file_path = target_dir.joinpath(config_name)
            for mapping in file_map:
                if mapping["file"] == config_name:
                    target_file_path = target_dir.joinpath(Path(mapping["link"]))

            if target_file_path.is_symlink():
                link_dest = os.readlink(target_file_path)
                resolved_dest = str(Path(os.path.realpath(target_file_path)))
                source_resolved = str(Path(source_file_path).resolve())

                # Verify that the symlink actually belongs to this dotfile source
                if resolved_dest == source_resolved or source_dir.name in link_dest:
                    typer.echo(f"{source_dir}: Removing link: {target_file_path} -> {link_dest}")
                    os.unlink(target_file_path)
                else:
                    typer.echo(f"{source_dir}: Skipping {target_file_path} (points to {link_dest}, not {source_file_path})")

        # Also recursively clean up any nested symlinks pointing into source_dir (e.g. manual lua/tarberd links)
        if target_dir.is_dir():
            for root, dirs, files in os.walk(target_dir, topdown=False):
                current_dir = Path(root)
                for item in list(dirs) + list(files):
                    p = current_dir / item
                    if p.is_symlink():
                        try:
                            link_target = os.readlink(p)
                            resolved = str(p.resolve())
                            if str(source_dir.resolve()) in resolved or "dotfiles" in link_target:
                                typer.echo(f"{source_dir}: Removing nested link: {p} -> {link_target}")
                                p.unlink()
                        except OSError:
                            pass
                # If directory became empty after unlinking, remove it cleanly
                try:
                    if current_dir != target_dir:
                        current_dir.rmdir()
                except OSError:
                    pass

