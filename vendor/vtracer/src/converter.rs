use super::config::{ColorMode, Config, ConverterConfig, Hierarchical};
use super::svg::SvgFile;
use fastrand::Rng;
use visioncortex::color_clusters::{KeyingAction, Runner, RunnerConfig, HIERARCHICAL_MAX};
use visioncortex::{Color, ColorImage, ColorName, CompoundPath, CompoundPathElement};

const NUM_UNUSED_COLOR_ITERATIONS: usize = 6;
/// The fraction of pixels in the top/bottom rows of the image that need to be transparent before
/// the entire image will be keyed.
const KEYING_THRESHOLD: f32 = 0.2;

/// Work limits for in-memory tracing. Cluster limits are checked between batches;
/// a single batch and a single generated path can exceed the limit temporarily.
#[derive(Clone, Copy)]
pub struct TraceLimits {
    pub max_edges: usize,
    pub max_clusters: usize,
    pub max_paths: usize,
    pub max_points: usize,
}

/// Convert an in-memory image into an in-memory SVG.
pub fn convert(img: ColorImage, config: Config) -> Result<SvgFile, String> {
    convert_impl(img, config, None)
}

/// Convert with complexity budgets before clustering and while generating paths.
pub fn convert_bounded(
    img: ColorImage,
    config: Config,
    limits: TraceLimits,
) -> Result<SvgFile, String> {
    check_edges(
        &img,
        limits.max_edges,
        matches!(config.color_mode, ColorMode::Binary),
    )?;
    convert_impl(img, config, Some(limits))
}

fn convert_impl(
    img: ColorImage,
    config: Config,
    limits: Option<TraceLimits>,
) -> Result<SvgFile, String> {
    let config = config.into_converter_config();
    match config.color_mode {
        ColorMode::Color => color_image_to_svg(img, config, limits),
        ColorMode::Binary => binary_image_to_svg(img, config, limits),
    }
}

fn check_edges(img: &ColorImage, max_edges: usize, binary: bool) -> Result<(), String> {
    let mut edges = 0usize;
    for y in 0..img.height {
        for x in 0..img.width {
            let pixel = img.get_pixel(x, y);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x, y.wrapping_sub(1))] {
                if nx >= img.width || ny >= img.height {
                    continue;
                }
                let other = img.get_pixel(nx, ny);
                if if binary {
                    (pixel.r < 128) != (other.r < 128)
                } else {
                    pixel.r.abs_diff(other.r) > 1
                        || pixel.g.abs_diff(other.g) > 1
                        || pixel.b.abs_diff(other.b) > 1
                } {
                    edges += 1;
                    if edges > max_edges {
                        return Err(format!("image exceeds the {max_edges}-edge tracing limit"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn run_clusters(
    runner: Runner,
    limits: Option<TraceLimits>,
) -> Result<visioncortex::color_clusters::Clusters, String> {
    if let Some(limits) = limits {
        let mut builder = runner.start();
        loop {
            let done = builder.tick();
            if builder.view().clusters.len() > limits.max_clusters {
                return Err(format!(
                    "image exceeds the {}-cluster tracing limit",
                    limits.max_clusters
                ));
            }
            if done {
                return Ok(builder.result());
            }
        }
    }
    Ok(runner.run())
}

fn admit_path(
    svg: &SvgFile,
    path: &CompoundPath,
    points: &mut usize,
    limits: Option<TraceLimits>,
) -> Result<(), String> {
    if let Some(limits) = limits {
        if svg.paths.len() >= limits.max_paths {
            return Err(format!(
                "image exceeds the {}-path tracing limit",
                limits.max_paths
            ));
        }
        let added = path
            .paths
            .iter()
            .try_fold(0usize, |total, part| {
                let count = match part {
                    CompoundPathElement::PathI32(path) => path.len(),
                    CompoundPathElement::PathF64(path) => path.len(),
                    CompoundPathElement::Spline(path) => path.len(),
                };
                total.checked_add(count)
            })
            .ok_or("tracing point count overflowed")?;
        *points = points
            .checked_add(added)
            .ok_or("tracing point count overflowed")?;
        if *points > limits.max_points {
            return Err(format!(
                "image exceeds the {}-point tracing limit",
                limits.max_points
            ));
        }
    }
    Ok(())
}

fn color_exists_in_image(img: &ColorImage, color: Color) -> bool {
    for y in 0..img.height {
        for x in 0..img.width {
            let pixel_color = img.get_pixel(x, y);
            if pixel_color.r == color.r && pixel_color.g == color.g && pixel_color.b == color.b {
                return true;
            }
        }
    }
    false
}

fn find_unused_color_in_image(img: &ColorImage) -> Result<Color, String> {
    let special_colors = IntoIterator::into_iter([
        Color::new(255, 0, 0),
        Color::new(0, 255, 0),
        Color::new(0, 0, 255),
        Color::new(255, 255, 0),
        Color::new(0, 255, 255),
        Color::new(255, 0, 255),
    ]);
    let mut rng = Rng::new();
    let random_colors =
        (0..NUM_UNUSED_COLOR_ITERATIONS).map(|_| Color::new(rng.u8(..), rng.u8(..), rng.u8(..)));
    for color in special_colors.chain(random_colors) {
        if !color_exists_in_image(img, color) {
            return Ok(color);
        }
    }
    Err(String::from(
        "unable to find unused color in image to use as key",
    ))
}

fn should_key_image(img: &ColorImage) -> bool {
    if img.width == 0 || img.height == 0 {
        return false;
    }

    // Check for transparency at several scanlines
    let threshold = ((img.width * 2) as f32 * KEYING_THRESHOLD) as usize;
    let mut num_transparent_pixels = 0;
    let y_positions = [
        0,
        img.height / 4,
        img.height / 2,
        3 * img.height / 4,
        img.height - 1,
    ];
    for y in y_positions {
        for x in 0..img.width {
            if img.get_pixel(x, y).a == 0 {
                num_transparent_pixels += 1;
            }
            if num_transparent_pixels >= threshold {
                return true;
            }
        }
    }

    false
}

fn color_image_to_svg(
    mut img: ColorImage,
    config: ConverterConfig,
    limits: Option<TraceLimits>,
) -> Result<SvgFile, String> {
    let width = img.width;
    let height = img.height;

    let key_color = if should_key_image(&img) {
        let key_color = find_unused_color_in_image(&img)?;
        for y in 0..height {
            for x in 0..width {
                if img.get_pixel(x, y).a == 0 {
                    img.set_pixel(x, y, &key_color);
                }
            }
        }
        key_color
    } else {
        // The default color is all zeroes, which is treated by visioncortex as a special value meaning no keying will be applied.
        Color::default()
    };

    let runner = Runner::new(
        RunnerConfig {
            diagonal: config.layer_difference == 0,
            hierarchical: HIERARCHICAL_MAX,
            batch_size: if limits.is_some() { 1024 } else { 25600 },
            good_min_area: config.filter_speckle_area,
            good_max_area: (width * height),
            is_same_color_a: config.color_precision_loss,
            is_same_color_b: 1,
            deepen_diff: config.layer_difference,
            hollow_neighbours: 1,
            key_color,
            keying_action: if matches!(config.hierarchical, Hierarchical::Cutout) {
                KeyingAction::Keep
            } else {
                KeyingAction::Discard
            },
        },
        img,
    );

    let mut clusters = run_clusters(runner, limits)?;

    match config.hierarchical {
        Hierarchical::Stacked => {}
        Hierarchical::Cutout => {
            let view = clusters.view();
            let image = view.to_color_image();
            let runner = Runner::new(
                RunnerConfig {
                    diagonal: false,
                    hierarchical: 64,
                    batch_size: if limits.is_some() { 1024 } else { 25600 },
                    good_min_area: 0,
                    good_max_area: (image.width * image.height) as usize,
                    is_same_color_a: 0,
                    is_same_color_b: 1,
                    deepen_diff: 0,
                    hollow_neighbours: 0,
                    key_color,
                    keying_action: KeyingAction::Discard,
                },
                image,
            );
            clusters = run_clusters(runner, limits)?;
        }
    }

    let view = clusters.view();

    let mut svg = SvgFile::new(width, height, config.path_precision);
    let mut points = 0;
    for &cluster_index in view.clusters_output.iter().rev() {
        if limits.is_some_and(|l| svg.paths.len() >= l.max_paths) {
            return Err("image exceeds the path tracing limit".into());
        }
        let cluster = view.get_cluster(cluster_index);
        let paths = cluster.to_compound_path(
            &view,
            false,
            config.mode,
            config.corner_threshold,
            config.length_threshold,
            config.max_iterations,
            config.splice_threshold,
        );
        admit_path(&svg, &paths, &mut points, limits)?;
        svg.add_path(paths, cluster.residue_color());
    }

    Ok(svg)
}

fn binary_image_to_svg(
    img: ColorImage,
    config: ConverterConfig,
    limits: Option<TraceLimits>,
) -> Result<SvgFile, String> {
    let img = img.to_binary_image(|x| x.r < 128);
    let width = img.width;
    let height = img.height;

    let clusters = img.to_clusters(false);
    if limits.is_some_and(|l| clusters.len() > l.max_clusters) {
        return Err("image exceeds the cluster tracing limit".into());
    }

    let mut svg = SvgFile::new(width, height, config.path_precision);
    let mut points = 0;
    for i in 0..clusters.len() {
        let cluster = clusters.get_cluster(i);
        if cluster.size() >= config.filter_speckle_area {
            if limits.is_some_and(|l| svg.paths.len() >= l.max_paths) {
                return Err("image exceeds the path tracing limit".into());
            }
            let paths = cluster.to_compound_path(
                config.mode,
                config.corner_threshold,
                config.length_threshold,
                config.max_iterations,
                config.splice_threshold,
            );
            admit_path(&svg, &paths, &mut points, limits)?;
            svg.add_path(paths, Color::color(&ColorName::Black));
        }
    }

    Ok(svg)
}
