use assets_manager::source::Source;
struct Obj {
    pub models: Vec<tobj::Model>,
    pub materials: Vec<tobj::Material>,
}
#[allow(clippy::upper_case_acronyms)]
struct TMTL(tobj::MTLLoadResult);
struct MtlLoader();
impl assets_manager::Asset for TMTL {
    type Loader = MtlLoader;
    const EXTENSION: &'static str = "mtl";
}
impl assets_manager::loader::Loader<TMTL> for MtlLoader {
    fn load(
        content: std::borrow::Cow<[u8]>,
        _ext: &str,
    ) -> Result<TMTL, assets_manager::BoxedError> {
        let mut reader = std::io::BufReader::new(content.as_ref());
        Ok(TMTL(tobj::load_mtl_buf(&mut reader)))
    }
}

impl Obj {
    fn textures(&self) -> impl Iterator<Item = &String> {
        self.materials.iter().flat_map(|mtl| {
            mtl.ambient_texture
                .iter()
                .chain(mtl.diffuse_texture.iter())
                .chain(mtl.specular_texture.iter())
                .chain(mtl.normal_texture.iter())
                .chain(mtl.shininess_texture.iter())
                .chain(mtl.dissolve_texture.iter())
        })
    }
}
fn path_to_id(base_id: &str, path: &std::path::Path) -> (String, String) {
    let ext = path
        .extension()
        .map(|osstr| osstr.to_string_lossy())
        .unwrap_or_default();
    let path = path.with_extension("");
    let mut path_id = String::with_capacity(path.capacity() + base_id.len() + 1);
    if base_id.is_empty() {
        path_id += base_id;
        path_id += ".";
    }
    let mut comps = path.components().peekable();
    while let Some(comp) = comps.next() {
        path_id += &comp.as_os_str().to_string_lossy();
        if comps.peek().is_some() {
            path_id += ".";
        }
    }
    (path_id, ext.into_owned())
}
impl assets_manager::Compound for Obj {
    fn load(
        cache: assets_manager::AnyCache,
        id: &assets_manager::SharedString,
    ) -> Result<Self, assets_manager::BoxedError> {
        let source = cache.raw_source();
        let base_id = match id.rfind('.') {
            Some(index) => &id[..index],
            None => "",
        };
        let obj_bytes = source.read(id, "obj")?;
        let (models, materials) = tobj::load_obj_buf(
            &mut std::io::BufReader::new(obj_bytes.as_ref()),
            &tobj::LoadOptions {
                single_index: true,
                ..Default::default()
            },
            |path| {
                let (path_id, _ext) = path_to_id(base_id, path);
                cache.load::<TMTL>(&path_id).unwrap().read().0.clone()
            },
        )?;
        let mut obj = Obj {
            models,
            materials: materials.map_err(assets_manager::BoxedError::from)?,
        };
        // replace paths in texture names with asset IDs
        for mtl in obj.materials.iter_mut() {
            mtl.ambient_texture
                .iter_mut()
                .chain(mtl.diffuse_texture.iter_mut())
                .chain(mtl.specular_texture.iter_mut())
                .chain(mtl.normal_texture.iter_mut())
                .chain(mtl.shininess_texture.iter_mut())
                .chain(mtl.dissolve_texture.iter_mut())
                .try_for_each(|tex| {
                    let (path_id, _ext) = path_to_id(base_id, std::path::Path::new(tex));
                    // also load the image to mark it as a dependency
                    let ret = cache
                        .load::<assets_manager::asset::Png>(&path_id)
                        .map(|_| ());
                    *tex = path_id;
                    ret
                })?;
        }
        Ok(obj)
    }
}
