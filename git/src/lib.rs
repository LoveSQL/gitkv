pub extern crate git2;

use git2::{Error, Repository};
use std::{collections::HashMap, fs, io, path::Path};

pub trait GitOps {
    fn cat_file(
        &self,
        repo: &Repository,
        reference: &str,
        filename: &str,
    ) -> Result<Vec<u8>, Error>;
}

pub struct LibGitOps;

impl GitOps for LibGitOps {
    /// Given an existing git repository, it will read the blob that the reference and the filename
    /// point to and return it as a String.
    fn cat_file(
        &self,
        repo: &Repository,
        reference: &str,
        filename: &str,
    ) -> Result<Vec<u8>, Error> {
        let reference = repo.find_reference(reference)?;
        let tree = reference.peel_to_tree()?;
        let path = std::path::Path::new(filename);
        let te = tree.get_path(path)?;

        repo.find_blob(te.id()).map(|x| x.content().to_owned())
    }
}

pub fn load_repos(root_path: &Path) -> Result<HashMap<String, Repository>, io::Error> {
    let mut repos = HashMap::new();
    if root_path.is_dir() {
        for entry in fs::read_dir(root_path)? {
            let path = entry.map(|x| x.path())?;
            if path.is_dir() {
                if let Some(fstem) = path.to_owned().file_stem() {
                    // Just ignoring any error opening the repo as it means a directory is
                    // not a valid git repo.
                    let _ = Repository::open(path).map(|repo| {
                        repos.insert(fstem.to_os_string().into_string().unwrap(), repo) //TODO remove unwrap
                    });
                }
            }
        }
    }
    Ok(repos)
}

#[cfg(test)]
mod tests {

    extern crate tempdir;

    use super::{GitOps, LibGitOps};

    use git2::Repository;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn git_cat_file(
        repo_path: &Repository,
        reference: &str,
        filename: &str,
    ) -> Result<Vec<u8>, git2::Error> {
        let gh = LibGitOps {};
        gh.cat_file(repo_path, reference, filename)
    }

    fn git_cat_file_err(repo_path: &Repository, reference: &str, filename: &str) -> git2::Error {
        git_cat_file(repo_path, reference, filename).expect_err("should be an error")
    }

    #[test]
    fn test_cat_file_with_existing_ref_and_file() {
        with_repo("file content", "dir/existing.file", |repo| {
            let res =
                git_cat_file(repo, "refs/heads/master", "dir/existing.file").expect("should be ok");
            assert_eq!(
                std::str::from_utf8(&res).expect("valid utf8"),
                "file content"
            );
        })
    }

    #[test]
    fn test_cat_file_with_non_existing_ref() {
        with_repo("file content", "dir/existing.file", |repo| {
            let res = git_cat_file_err(repo, "refs/heads/non-existing", "dir/existing.file");
            assert_eq!(res.code(), git2::ErrorCode::NotFound);
            assert_eq!(res.class(), git2::ErrorClass::Reference);
        })
    }

    #[test]
    fn test_cat_file_with_non_existing_file() {
        with_repo("file content", "dir/existing.file", |repo| {
            let res = git_cat_file_err(repo, "refs/heads/master", "non-existing.file");
            assert_eq!(res.code(), git2::ErrorCode::NotFound);
            assert_eq!(res.class(), git2::ErrorClass::Tree);
        })
    }

    #[test]
    fn test_cat_file_with_dir() {
        with_repo("content", "dir/existing.file", |repo| {
            let res = git_cat_file_err(repo, "refs/heads/master", "dir");
            assert_eq!(res.code(), git2::ErrorCode::NotFound);
            assert_eq!(res.class(), git2::ErrorClass::Invalid);
        })
    }

    pub fn with_repo<F>(file_contents: &str, file: &str, callback: F)
    where
        F: Fn(&Repository),
    {
        let dir = tempdir::TempDir::new("testgitrepo").expect("can't create tmp dir");

        let repo = Repository::init(&dir).expect("can't initialise repository");

        let path = dir.path().join(file);
        path.parent().map(|parent| fs::create_dir_all(&parent));
        fs::File::create(path)
            .and_then(|mut file| file.write_all(file_contents.as_bytes()))
            .expect("can't write file contents");

        repo.index()
            .and_then(|mut index| {
                index
                    .add_path(Path::new(file))
                    .expect("can't add file to index");
                repo.signature().and_then(|sig| {
                    index
                        .write_tree()
                        .and_then(|tid| repo.find_tree(tid))
                        .and_then(|tree| {
                            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                        })
                })
            })
            .expect("can't do first commit");

        callback(&repo);
        dir.close().expect("couldn't close the dir");
    }

}
