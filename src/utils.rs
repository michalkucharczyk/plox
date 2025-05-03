use std::path::PathBuf;

pub fn common_path_ancestor(paths: &[PathBuf]) -> Option<PathBuf> {
	let canonicalized: Result<Vec<_>, _> = paths.iter().map(|p| p.canonicalize()).collect();
	common_path_ancestor_inner(&canonicalized.ok()?)
}

fn common_path_ancestor_inner(paths: &[PathBuf]) -> Option<PathBuf> {
	if paths.is_empty() {
		return None;
	}

	let mut iter = paths.iter();
	let first = iter.next()?;

	let mut components: Vec<_> =
		first.parent().expect("shall be full path here").components().collect();

	for path in iter {
		let mut new_components = Vec::new();
		for (a, b) in components
			.iter()
			.zip(path.parent().expect("shall be full path here").components())
		{
			if a == &b {
				new_components.push(*a);
			} else {
				break;
			}
		}
		if new_components.is_empty() {
			return None;
		}
		components = new_components;
	}

	let ancestor = components.iter().fold(PathBuf::new(), |mut acc, comp| {
		acc.push(comp.as_os_str());
		acc
	});

	Some(ancestor)
}

#[cfg(test)]
mod tests {
	use crate::{logging::init_tracing_test, utils::common_path_ancestor_inner};
	use std::path::PathBuf;

	#[test]
	fn test_common_path_ancestor() {
		init_tracing_test();
		let p1 = PathBuf::from("/a/b/c/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b"));
		let p1 = PathBuf::from("/a/b/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b/d"));
		let p1 = PathBuf::from("/a/b/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b/d"));
		let p1 = PathBuf::from("/a/c/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a"));
		let p1 = PathBuf::from("/a/c/d/log1");
		let r = common_path_ancestor_inner(&[p1]).unwrap();
		assert_eq!(r, PathBuf::from("/a/c/d/"));
		let p1 = PathBuf::from("/log1");
		let r = common_path_ancestor_inner(&[p1]).unwrap();
		assert_eq!(r, PathBuf::from("/"));
		let p1 = PathBuf::from("/log1");
		let p2 = PathBuf::from("/log2");
		let r = common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/"));
	}
}
