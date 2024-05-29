use dav_server::{davpath::DavPath, fs::*, localfs::LocalFs};
use futures::{future::ready, StreamExt, TryFutureExt};
use std::path::{Component, Path};

#[derive(Clone)]
pub struct RestrictedFs {
    local: Box<LocalFs>,
    allow_hidden: bool,
}

impl RestrictedFs {
    pub fn new<P: AsRef<Path>>(base: P, allow_hidden: bool) -> Box<RestrictedFs> {
        let local = LocalFs::new(base, false, false, false);
        Box::new({
            RestrictedFs {
                local,
                allow_hidden,
            }
        })
    }
}

fn check_path(path: &DavPath) -> bool {
    path.as_pathbuf().components().all(|c| match c {
        Component::Normal(name) => name.to_str().map_or(false, |s| !s.starts_with('.')),
        _ => true,
    })
}

impl DavFileSystem for RestrictedFs {
    fn open<'a>(&'a self, path: &'a DavPath, options: OpenOptions) -> FsFuture<Box<dyn DavFile>> {
        if self.allow_hidden || check_path(path) {
            self.local.open(path, options)
        } else {
            Box::pin(ready(Err(FsError::NotFound)))
        }
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        meta: ReadDirMeta,
    ) -> FsFuture<FsStream<Box<dyn DavDirEntry>>> {
        if self.allow_hidden {
            self.local.read_dir(path, meta)
        } else if check_path(path) {
            Box::pin(self.local.read_dir(path, meta).map_ok(|stream| {
                let dyn_stream: FsStream<Box<dyn DavDirEntry>> =
                    Box::pin(stream.filter(|entry| ready(!entry.name().starts_with(b"."))));
                dyn_stream
            }))
        } else {
            Box::pin(ready(Err(FsError::NotFound)))
        }
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        if self.allow_hidden || check_path(path) {
            self.local.metadata(path)
        } else {
            Box::pin(ready(Err(FsError::NotFound)))
        }
    }

    fn symlink_metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        if self.allow_hidden || check_path(path) {
            self.local.symlink_metadata(path)
        } else {
            Box::pin(ready(Err(FsError::NotFound)))
        }
    }
}
