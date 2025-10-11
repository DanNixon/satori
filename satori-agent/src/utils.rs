use std::{fs, path::Path};

pub(crate) fn get_size<P>(path: P) -> std::io::Result<u64>
where
    P: AsRef<Path>,
{
    let mut result: u64 = 0;

    for entry in fs::read_dir(&path)? {
        let path = entry?.path();

        if path.is_file() {
            result += path.metadata()?.len();
        } else {
            result += get_size(path)?;
        }
    }

    Ok(result)
}
