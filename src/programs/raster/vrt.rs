use crate::{
    errors::*,
    utils::{_last_null_pointer_err, _path_to_c_string},
    Dataset,
};
use gdal_sys::GDALBuildVRTOptions;
use libc::{c_char, c_int};
use std::{
    borrow::Borrow,
    ffi::CString,
    path::Path,
    ptr::{null, null_mut},
};

/// Wraps a [GDALBuildVRTOptions] object.
///
/// [GDALBuildVRTOptions]: https://gdal.org/api/gdal_utils.html#_CPPv419GDALBuildVRTOptions
pub struct BuildVRTOptions {
    c_options: *mut GDALBuildVRTOptions,
}

impl BuildVRTOptions {
    /// See [GDALBuildVRTOptionsNew].
    ///
    /// [GDALBuildVRTOptionsNew]: https://gdal.org/api/gdal_utils.html#_CPPv422GDALBuildVRTOptionsNewPPcP28GDALBuildVRTOptionsForBinary
    pub fn new<S: Into<Vec<u8>>, I: IntoIterator<Item=S>>(args: I) -> Result<Self> {
        // Convert args to CStrings to add terminating null bytes
        let cstr_args = args
            .into_iter()
            .map(CString::new)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Get pointers to the strings
        // These strings don't actually get modified, the C API is just not const-correct
        // Null-terminate the list
        let mut c_args = cstr_args
            .iter()
            .map(|x| x.as_ptr() as *mut c_char)
            .chain(std::iter::once(null_mut()))
            .collect::<Vec<_>>();

        unsafe {
            Ok(Self {
                c_options: gdal_sys::GDALBuildVRTOptionsNew(c_args.as_mut_ptr(), null_mut()),
            })
        }
    }

    /// Returns the wrapped C pointer
    ///
    /// # Safety
    /// This method returns a raw C pointer
    pub unsafe fn c_options(&self) -> *mut GDALBuildVRTOptions {
        self.c_options
    }
}

impl Drop for BuildVRTOptions {
    fn drop(&mut self) {
        unsafe {
            gdal_sys::GDALBuildVRTOptionsFree(self.c_options);
        }
    }
}

// helper for distinguishing betweeen invocation modes.
enum DSSpec<'a> {
    DS(Vec<&'a Dataset>),
    Path(Vec<&'a Path>),
}

/// Build a VRT from a list of datasets.
/// Wraps [GDALBuildVRT].
/// See the [program docs] for more details.
///
/// [GDALBuildVRT]: https://gdal.org/api/gdal_utils.html#gdal__utils_8h_1a057aaea8b0ed0476809a781ffa377ea4
/// [program docs]: https://gdal.org/programs/gdalbuildvrt.html
pub fn build_vrt<D: Borrow<Dataset>>(
    dest: Option<&Path>,
    datasets: &[D],
    options: Option<BuildVRTOptions>,
) -> Result<Dataset> {
    let spec = DSSpec::DS(datasets
        .iter()
        .map(|x| x.borrow())
        .collect());

    _build_vrt(
        dest,
        &spec,
        options,
    )
}

/// Build a VRT from a list of dataset names (e.g. file paths).
///
/// Wraps [GDALBuildVRT].
/// See the [program docs] for more details.
///
/// [GDALBuildVRT]: https://gdal.org/api/gdal_utils.html#gdal__utils_8h_1a057aaea8b0ed0476809a781ffa377ea4
/// [program docs]: https://gdal.org/programs/gdalbuildvrt.html
pub fn build_vrt_from_paths<P: AsRef<Path>>(
    dest: Option<&Path>,
    dataset_paths: Vec<P>,
    options: Option<BuildVRTOptions>,
) -> Result<Dataset> {
    let spec = DSSpec::Path(dataset_paths
        .iter()
        .map(|x| x.as_ref())
        .collect());

    _build_vrt(
        dest,
        &spec,
        options,
    )
}

fn _build_vrt(
    dest: Option<&Path>,
    datasets: &DSSpec,
    options: Option<BuildVRTOptions>,
) -> Result<Dataset> {
    // Convert dest to CString
    let dest = dest.map(_path_to_c_string).transpose()?;
    let c_dest = dest.as_ref().map(|x| x.as_ptr()).unwrap_or(null());

    let c_options = options
        .as_ref()
        .map(|x| x.c_options as *const GDALBuildVRTOptions)
        .unwrap_or(null());

    let (src_count, src_datasets, src_dataset_names) = match datasets {
        DSSpec::DS(datasets) => {
            // Get raw handles to the datasets
            let mut datasets_raw: Vec<gdal_sys::GDALDatasetH> = unsafe {
                datasets.iter()
                    .map(|x| x.c_dataset())
                    .collect::<Vec<_>>()
            };
            (datasets_raw.len(), datasets_raw.as_mut_ptr(), null())
        }
        DSSpec::Path(paths) => {
            // Convert paths into C character array pointers
            let c_paths = paths.iter()
                .map(|&p| _path_to_c_string(p))
                .map(|cp| cp.map(|p| p.as_ptr()))
                .collect::<Result<Vec<_>>>()?;
            (c_paths.len(), null_mut(), c_paths.as_ptr())
        }
    };

    let dataset_out = unsafe {
        gdal_sys::GDALBuildVRT(
            c_dest, // the destination dataset path.
            src_count as c_int, // the number of input datasets.
            src_datasets, // the list of input datasets (or NULL, exclusive with papszSrcDSNames)
            src_dataset_names, // the list of input dataset names (or NULL, exclusive with pahSrcDS)
            c_options, // the options struct returned by GDALBuildVRTOptionsNew() or NULL.
            null_mut(),
        )
    };

    if dataset_out.is_null() {
        return Err(_last_null_pointer_err("GDALBuildVRT"));
    }

    let result = unsafe { Dataset::from_c_dataset(dataset_out) };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::{env, fs};
    use std::path::Path;
    use crate::{Dataset, errors};
    use crate::programs::raster::build_vrt;

    #[test]
    fn vrt_from_ds_and_path() -> errors::Result<()> {
        let infile = Path::new("fixtures/m_3607824_se_17_1_20160620_sub.tif");
        let ds = Dataset::open(infile)?;
        let outfile = env::temp_dir().join("test.vrt");
        {
            let vrt = build_vrt(Some(&outfile), &[&ds], None)?;
            assert_eq!(vrt.raster_count(), ds.raster_count());
        }

        let result = fs::read_to_string(outfile).unwrap();
        println!("{result}");

        Ok(())
    }
}