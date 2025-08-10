//! Helper types and functions to allow configuring hidden files visibility
//! for WebDAV handlers

use dav_server::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError as DavFsError,
        FsFuture as DavFsFuture, FsStream as DavFsStream, OpenOptions as DavOpenOptions,
        ReadDirMeta as DavReadDirMeta,
    },
    localfs::LocalFs,
};
use futures::StreamExt;
use std::ffi::OsStr;
#[cfg(target_family = "unix")]
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use tokio::fs;

/// A dav_server local filesystem backend that can be configured to deny access
/// to files and directories with names starting with a dot.
#[derive(Clone)]
pub struct RestrictedFs {
    local: Box<LocalFs>,
    base_path: PathBuf,
    show_hidden: bool,
    no_symlinks: bool,
}

impl RestrictedFs {
    /// Creates a new RestrictedFs serving the local path at "base".
    /// If "show_hidden" is false, access to hidden files is prevented.
    /// If "no_symlinks" is true, access to symlinks is prevented.
    pub fn new<P: AsRef<Path>>(base: P, show_hidden: bool, no_symlinks: bool) -> Box<RestrictedFs> {
        let base_path = base.as_ref().to_path_buf();
        let local = LocalFs::new(base, false, false, false);
        Box::new(RestrictedFs {
            local,
            base_path,
            show_hidden,
            no_symlinks,
        })
    }

    /// true if the path is allowed to appear in responses (not hidden and/or not a symlink, depending on flags)
    async fn is_path_allowed(&self, path: &DavPath) -> bool {
        if self.no_symlinks && path_has_symlink_components(path, &self.base_path).await {
            return false;
        }
        if !self.show_hidden && path_has_hidden_components(path) {
            return false;
        }
        true
    }
}

/// true if any normal component of path either starts with dot or can't be turned into a str
fn path_has_hidden_components(path: &DavPath) -> bool {
    path.as_rel_ospath().components().any(|c| match c {
        Component::Normal(name) => name.to_str().is_none_or(|s| s.starts_with('.')),
        _ => panic!("dav path should not contain any non-normal components"),
    })
}

/// true if any component in `path` (relative to `base_path`) is a symlink
async fn path_has_symlink_components(path: &DavPath, base_path: &Path) -> bool {
    let mut current_path = base_path.to_path_buf();
    for comp in path.as_rel_ospath().components() {
        match comp {
            Component::Normal(name) => {
                current_path.push(name);
                if let Ok(md) = fs::symlink_metadata(&current_path).await
                    && md.file_type().is_symlink()
                {
                    return true;
                }
            }
            _ => {
                panic!("dav path should not contain any non-normal components")
            }
        }
    }
    false
}

impl DavFileSystem for RestrictedFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: DavOpenOptions,
    ) -> DavFsFuture<'a, Box<dyn DavFile>> {
        Box::pin(async move {
            if !self.is_path_allowed(path).await {
                Err(DavFsError::NotFound)
            } else {
                self.local.open(path, options).await
            }
        })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        meta: DavReadDirMeta,
    ) -> DavFsFuture<'a, DavFsStream<Box<dyn DavDirEntry>>> {
        Box::pin(async move {
            if !self.is_path_allowed(path).await {
                return Err(DavFsError::NotFound);
            }

            if self.show_hidden && !self.no_symlinks {
                return self.local.read_dir(path, meta).await;
            }

            let dav_path = path.as_rel_ospath();
            let base_path = self.base_path.join(dav_path);
            let show_hidden = self.show_hidden;
            let no_symlinks = self.no_symlinks;

            let stream = self.local.read_dir(path, meta).await?;

            let filtered = stream.filter_map(move |entry_res| {
                let base_path = base_path.clone();
                async move {
                    match entry_res {
                        Ok(e) => {
                            if !show_hidden && e.name().starts_with(b".") {
                                return None;
                            }
                            if no_symlinks {
                                let name = e.name();
                                #[cfg(not(target_os = "windows"))]
                                let os_string = OsStr::from_bytes(&name);
                                #[cfg(target_os = "windows")]
                                let os_string: &OsStr =
                                    std::str::from_utf8(&name).unwrap().as_ref();
                                let entry_path = base_path.join(os_string);
                                if let Ok(md) = fs::symlink_metadata(&entry_path).await
                                    && md.file_type().is_symlink()
                                {
                                    return None;
                                }
                            }
                            Some(Ok(e))
                        }
                        Err(e) => Some(Err(e)),
                    }
                }
            });

            Ok(Box::pin(filtered) as DavFsStream<Box<dyn DavDirEntry>>)
        })
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> DavFsFuture<'a, Box<dyn DavMetaData>> {
        Box::pin(async move {
            if !self.is_path_allowed(path).await {
                Err(DavFsError::NotFound)
            } else {
                self.local.metadata(path).await
            }
        })
    }

    fn symlink_metadata<'a>(&'a self, path: &'a DavPath) -> DavFsFuture<'a, Box<dyn DavMetaData>> {
        Box::pin(async move {
            if !self.is_path_allowed(path).await {
                Err(DavFsError::NotFound)
            } else {
                self.local.symlink_metadata(path).await
            }
        })
    }
}
