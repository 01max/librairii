"""Own a fresh CI-only Metal cache alias without granting external writes."""
import os
from pathlib import Path
import stat

BUNDLE = 'io.github.librairii.native'
MARKER = b'librairii-packaged-acceptance-v1\n'


def identity(value):
    return {'device': value.st_dev, 'inode': value.st_ino}


class NativeGraphicsCache:
    def __init__(self, root, parent):
        self.root, self.parent = Path(root), Path(parent)
        self.target = self.root/'native-graphics-cache'
        self.fd = self.target_fd = None
        self.root_identity = None
        self.parent_identity = self.target_identity = self.link_identity = None
        self.reserved, self.retired = False, False

    def prepare(self):
        if self.fd is not None or self.retired:
            raise ValueError('native cache owner cannot be prepared twice')
        marker = self.root/'.librairii-packaged-fixture'
        if (not self.root.is_absolute() or self.root.resolve(strict=True) != self.root
                or marker.is_symlink() or marker.stat().st_size != len(MARKER)
                or marker.read_bytes() != MARKER or not self.parent.is_absolute()
                or self.parent.resolve(strict=True) != self.parent):
            raise ValueError('native cache requires canonical owned fixture and cache parent')
        self.root_identity = identity(self.root.stat())
        self.fd = os.open(self.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.parent_identity = identity(os.fstat(self.fd))
        self.target.mkdir()  # Empty for every fresh/upgraded lane; never reuse a cache.
        self.target_fd = os.open(self.target, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.target_identity = identity(os.fstat(self.target_fd))
        # symlink is exclusive even for a dangling pre-existing entry. Register
        # reservation before later checks so uncertainty can never erase custody.
        os.symlink(str(self.target), BUNDLE, dir_fd=self.fd)
        self.reserved = True
        self.link_identity = identity(os.stat(BUNDLE, dir_fd=self.fd, follow_symlinks=False))
        self.verify()

    def verify(self):
        if (self.fd is None or not self.reserved
                or self.root.is_symlink() or identity(self.root.stat()) != self.root_identity
                or self.parent.is_symlink()
                or identity(self.parent.stat()) != self.parent_identity
                or identity(os.fstat(self.fd)) != self.parent_identity
                or self.target.is_symlink()
                or identity(self.target.stat()) != self.target_identity
                or identity(os.fstat(self.target_fd)) != self.target_identity):
            raise ValueError('native cache parent or owned target identity changed')
        link = os.stat(BUNDLE, dir_fd=self.fd, follow_symlinks=False)
        if (not stat.S_ISLNK(link.st_mode) or identity(link) != self.link_identity
                or os.readlink(BUNDLE, dir_fd=self.fd) != str(self.target)):
            raise ValueError('native cache reservation changed; retain anchor and fixture')

    def retire(self, process_retired):
        if self.reserved:
            if not process_retired:
                raise ValueError('native cache retained while product retirement is uncertain')
            self.verify()
            os.unlink(BUNDLE, dir_fd=self.fd)
            self.reserved = False
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None
        if self.target_fd is not None:
            os.close(self.target_fd)
            self.target_fd = None
        self.retired = True

    def receipt(self):
        return {'format': 'librairii.native-graphics-cache-custody', 'schema': 1,
                'anchor': str(self.parent/BUNDLE), 'target': str(self.target),
                'root_identity': self.root_identity, 'parent_identity': self.parent_identity,
                'target_identity': self.target_identity,
                'link_identity': self.link_identity, 'reserved': self.reserved,
                'retired': self.retired, 'sandbox_policy_changed': False}
