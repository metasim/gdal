use crate::vector::Geometry;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// A struct that contains a temporary directory and a path to a file in that directory.
pub struct TempFixture {
    _temp_dir: tempfile::TempDir,
    temp_path: PathBuf,
}

impl TempFixture {
    /// Creates a copy of the test file in a temporary directory.
    /// Returns the struct `TempFixture` that contains the temp dir (for clean-up on `drop`) as well as the path to the file.
    ///
    /// This can potentially be removed when <https://github.com/OSGeo/gdal/issues/6253> is resolved.
    pub fn fixture(name: &str) -> Self {
        let staging = Self::empty(name);
        let source = Path::new("fixtures").join(name);
        std::fs::copy(source, &staging.temp_path).unwrap();
        staging
    }

    /// Creates a temporary directory and path to a non-existent file with given `name`.
    /// Useful for writing results to during testing
    ///
    /// Returns the struct `TempFixture` that contains the temp dir (for clean-up on `drop`)
    /// as well as the empty file path.
    pub fn empty(name: &str) -> Self {
        let _temp_dir = tempfile::tempdir().unwrap();
        let temp_path = _temp_dir.path().join(name);
        Self {
            _temp_dir,
            temp_path,
        }
    }

    pub fn path(&self) -> &Path {
        &self.temp_path
    }
}

impl AsRef<Path> for TempFixture {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

/// Returns the fully qualified path to `filename` in `${CARGO_MANIFEST_DIR}/fixtures`.
pub(crate) fn fixture(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(filename)
}

/// Test for geometric equivalence: structurally the same and points equal within a tolerance.
///
/// Source: [`ogrtest.py`](https://github.com/OSGeo/gdal/blob/bef646f97ef043d5b34f2d85275d380574854a31/autotest/pymod/ogrtest.py#L82-L173)
pub(crate) fn assert_geom_equivalence(expected: &Geometry, test: &Geometry) {
    const EPSILON: f64 = 1e-8;
    // If structurally equal, consider testing done.
    if expected.eq(test) {
        return;
    }

    assert_eq!(
        expected.geometry_type(),
        test.geometry_type(),
        "geometry types do not match"
    );
    assert_eq!(
        expected.geometry_name(),
        test.geometry_name(),
        "geometry names do not match"
    );
    assert_eq!(
        expected.geometry_count(),
        test.geometry_count(),
        "sub-geometry counts do not match"
    );
    // Note: `point_count` returns `0` for non-line-like geometries.
    assert_eq!(
        expected.point_count(),
        test.point_count(),
        "geometry point counts do not match"
    );

    if expected.geometry_count() > 0 {
        for i in 0..expected.geometry_count() {
            assert_geom_equivalence(&expected.get_geometry(i), &test.get_geometry(i));
        }
    } else {
        fn check_dist(e: f64, t: f64, n: char, i: usize, eg: &Geometry, tg: &Geometry) {
            assert!(
                (e - t).abs() < EPSILON,
                "{n} coordinate of point {i} is not equivalent.\n\
            expected: {eg:?}\n\
            found:    {tg:?}"
            );
        }

        let points = expected.point_count();
        for i in 0..points {
            let (x1, y1, z1) = expected.get_point(i as i32);
            let (x2, y2, z2) = test.get_point(i as i32);
            check_dist(x1, x2, 'x', i, expected, test);
            check_dist(y1, y2, 'y', i, expected, test);
            check_dist(z1, z2, 'z', i, expected, test);
        }
    }
}

/// Scoped value for temporarily suppressing thread-local GDAL log messages.
///
/// Useful for tests that expect GDAL errors and want to keep the output log clean
/// of distracting yet expected error messages.
pub(crate) struct SuppressGDALErrorLog {
    // Make !Sync and !Send, and force use of `new`.
    _private: PhantomData<*mut c_void>,
}

impl SuppressGDALErrorLog {
    pub(crate) fn new() -> Self {
        unsafe { gdal_sys::CPLPushErrorHandler(Some(gdal_sys::CPLQuietErrorHandler)) };
        SuppressGDALErrorLog {
            _private: PhantomData,
        }
    }
}

impl Drop for SuppressGDALErrorLog {
    fn drop(&mut self) {
        unsafe { gdal_sys::CPLPopErrorHandler() };
    }
}
