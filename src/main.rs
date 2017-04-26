// Copyright (C) 2004, 2005 by Jason Bevins, 2017 Matthew Nicholls
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation; either version 2 of the License, or (at your option)
// any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
// (COPYING.txt) for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc., 59
// Temple Place, Suite 330, Boston, MA  02111-1307  USA
//
// The developer's email is jlbezigvins@gmzigail.com (for great email, take
// off every 'zig'.)
//

#[macro_use]
extern crate clap;
extern crate image;
extern crate noise;

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;
use std::thread::JoinHandle;

use clap::{App, Arg};
use image::ColorType;
use image::png::PNGEncoder;
use noise::module::{Add, Billow, Blend, Cache, Clamp, Constant, Curve, Exponent, Max, Min, Module,
                    Multiply, Perlin, RidgedMulti, ScaleBias, Select, Terrace, Turbulence, Voronoi};
use noise::noisegen::NoiseQuality;

////////////////////////////////////////////////////////////////////////////
// Constants
//
// Modify these constants to change the terrain of the planet and to change
// the boundaries and size of the elevation grid.
//
// Note: "Planetary elevation units" range from -1.0 (for the lowest
// underwater trenches) to +1.0 (for the highest mountain peaks.)
//

// Frequency of the planet's continents.  Higher frequency produces smaller,
// more numerous continents.  This value is measured in radians.
const CONTINENT_FREQUENCY: f64 = 1.0;

// Lacunarity of the planet's continents.  Changing this value produces
// slightly different continents.  For the best results, this value should
// be random, but close to 2.0.
const CONTINENT_LACUNARITY: f64 = 2.208984375;

// Lacunarity of the planet's mountains.  Changing this value produces
// slightly different mountains.  For the best results, this value should
// be random, but close to 2.0.
const MOUNTAIN_LACUNARITY: f64 = 2.142578125;

// Lacunarity of the planet's hills.  Changing this value produces slightly
// different hills.  For the best results, this value should be random, but
// close to 2.0.
const HILLS_LACUNARITY: f64 = 2.162109375;

// Lacunarity of the planet's plains.  Changing this value produces slightly
// different plains.  For the best results, this value should be random, but
// close to 2.0.
const PLAINS_LACUNARITY: f64 = 2.314453125;

// Lacunarity of the planet's badlands.  Changing this value produces
// slightly different badlands.  For the best results, this value should be
// random, but close to 2.0.
const BADLANDS_LACUNARITY: f64 = 2.212890625;

// Specifies the "twistiness" of the mountains.
const MOUNTAINS_TWIST: f64 = 1.0;

// Specifies the "twistiness" of the hills.
const HILLS_TWIST: f64 = 1.0;

// Specifies the "twistiness" of the badlands.
const BADLANDS_TWIST: f64 = 1.0;

// Specifies the planet's sea level.  This value must be between -1.0
// (minimum planet elevation) and +1.0 (maximum planet elevation.)
const SEA_LEVEL: f64 = 0.0;

// Specifies the level on the planet in which continental shelves appear.
// This value must be between -1.0 (minimum planet elevation) and +1.0
// (maximum planet elevation), and must be less than SEA_LEVEL.
const SHELF_LEVEL: f64 = -0.375;

// Determines the amount of mountainous terrain that appears on the
// planet.  Values range from 0.0 (no mountains) to 1.0 (all terrain is
// covered in mountains).  Mountainous terrain will overlap hilly terrain.
// Because the badlands terrain may overlap parts of the mountainous
// terrain, setting MOUNTAINS_AMOUNT to 1.0 may not completely cover the
// terrain in mountains.
const MOUNTAINS_AMOUNT: f64 = 0.5;

// Determines the amount of hilly terrain that appears on the planet.
// Values range from 0.0 (no hills) to 1.0 (all terrain is covered in
// hills).  This value must be less than MOUNTAINS_AMOUNT.  Because the
// mountainous terrain will overlap parts of the hilly terrain, and
// the badlands terrain may overlap parts of the hilly terrain, setting
// HILLS_AMOUNT to 1.0 may not completely cover the terrain in hills.
const HILLS_AMOUNT: f64 = (1.0 + MOUNTAINS_AMOUNT) / 2.0;

// Determines the amount of badlands terrain that covers the planet.
// Values range from 0.0 (no badlands) to 1.0 (all terrain is covered in
// badlands.)  Badlands terrain will overlap any other type of terrain.
const BADLANDS_AMOUNT: f64 = 0.03125;

// Offset to apply to the terrain type definition.  Low values (< 1.0) cause
// the rough areas to appear only at high elevations.  High values (> 2.0)
// cause the rough areas to appear at any elevation.  The percentage of
// rough areas on the planet are independent of this value.
const TERRAIN_OFFSET: f64 = 1.0;

// Specifies the amount of "glaciation" on the mountains.  This value
// should be close to 1.0 and greater than 1.0.
const MOUNTAIN_GLACIATION: f64 = 1.375;

// Scaling to apply to the base continent elevations, in planetary elevation
// units.
const CONTINENT_HEIGHT_SCALE: f64 = (1.0 - SEA_LEVEL) / 4.0;

// Maximum depth of the rivers, in planetary elevation units.
const RIVER_DEPTH: f64 = 0.0234375;

fn create_generator(seed: i32) -> Box<Module> {
    ////////////////////////////////////////////////////////////////////////////
    // Module group: continent definition
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: base continent definition (7 noise modules)
    //
    // This subgroup roughly defines the positions and base elevations of the
    // planet's continents.
    //
    // The "base elevation" is the elevation of the terrain before any terrain
    // features (mountains, hills, etc.) are placed on that terrain.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Continent module]: This Perlin-noise module generates the continents.
    //    This noise module has a high number of octaves so that detail is
    //    visible at high zoom levels.
    let mut base_continent_def_pe0 = Perlin::new();
    base_continent_def_pe0.set_seed(seed + 0);
    base_continent_def_pe0.set_frequency(CONTINENT_FREQUENCY);
    base_continent_def_pe0.set_persistence(0.5);
    base_continent_def_pe0.set_lacunarity(CONTINENT_LACUNARITY);
    base_continent_def_pe0.set_octave_count(14);
    base_continent_def_pe0.set_quality(NoiseQuality::Standard);

    // 2: [Continent-with-ranges module]: Next, a curve module modifies the
    //    output value from the continent module so that very high values appear
    //    near sea level.  This defines the positions of the mountain ranges.
    let mut base_continent_def_cu = Curve::new(base_continent_def_pe0.clone());
    base_continent_def_cu.add_control_point(-2.0000 + SEA_LEVEL, -1.625 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(-1.0000 + SEA_LEVEL, -1.375 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.0000 + SEA_LEVEL, -0.375 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.0625 + SEA_LEVEL, 0.125 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.1250 + SEA_LEVEL, 0.250 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.2500 + SEA_LEVEL, 1.000 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.5000 + SEA_LEVEL, 0.250 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(0.7500 + SEA_LEVEL, 0.250 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(1.0000 + SEA_LEVEL, 0.500 + SEA_LEVEL);
    base_continent_def_cu.add_control_point(2.0000 + SEA_LEVEL, 0.500 + SEA_LEVEL);

    // 3: [Carver module]: This higher-frequency Perlin-noise module will be
    //    used by subsequent noise modules to carve out chunks from the mountain
    //    ranges within the continent-with-ranges module so that the mountain
    //    ranges will not be complely impassible.
    let mut base_continent_def_pe1 = Perlin::new();
    base_continent_def_pe1.set_seed(seed + 1);
    base_continent_def_pe1.set_frequency(CONTINENT_FREQUENCY * 4.34375);
    base_continent_def_pe1.set_persistence(0.5);
    base_continent_def_pe1.set_lacunarity(CONTINENT_LACUNARITY);
    base_continent_def_pe1.set_octave_count(11);
    base_continent_def_pe1.set_quality(NoiseQuality::Standard);

    // 4: [Scaled-carver module]: This scale/bias module scales the output
    //    value from the carver module such that it is usually near 1.0.  This
    //    is required for step 5.
    let mut base_continent_def_sb = ScaleBias::new(base_continent_def_pe1.clone());
    base_continent_def_sb.set_scale(0.375);
    base_continent_def_sb.set_bias(0.625);

    // 5: [Carved-continent module]: This minimum-value module carves out chunks
    //    from the continent-with-ranges module.  It does this by ensuring that
    //    only the minimum of the output values from the scaled-carver module
    //    and the continent-with-ranges module contributes to the output value
    //    of this subgroup.  Most of the time, the minimum-value module will
    //    select the output value from the continents-with-ranges module since
    //    the output value from the scaled-carver module is usually near 1.0.
    //    Occasionally, the output value from the scaled-carver module will be
    //    less than the output value from the continent-with-ranges module, so
    //    in this case, the output value from the scaled-carver module is
    //    selected.
    let base_continent_def_mi = Min::new(base_continent_def_sb.clone(),
                                         base_continent_def_cu.clone());

    // 6: [Clamped-continent module]: Finally, a clamp module modifies the
    //    carved-continent module to ensure that the output value of this
    //    subgroup is between -1.0 and 1.0.
    let mut base_continent_def_cl = Clamp::new(base_continent_def_mi.clone());
    base_continent_def_cl.set_bounds(-1.0, 1.0);

    // 7: [Base-continent-definition subgroup]: Caches the output value from the
    //    clamped-continent module.
    let base_continent_def: Rc<Module> = Rc::new(Cache::new(base_continent_def_cl.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continent definition (5 noise modules)
    //
    // This subgroup warps the output value from the the base-continent-
    // definition subgroup, producing more realistic terrain.
    //
    // Warping the base continent definition produces lumpier terrain with
    // cliffs and rifts.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Coarse-turbulence module]: This turbulence module warps the output
    //    value from the base-continent-definition subgroup, adding some coarse
    //    detail to it.
    let mut continent_def_tu0 = Turbulence::new(base_continent_def.clone());
    continent_def_tu0.set_seed(seed + 10);
    continent_def_tu0.set_frequency(CONTINENT_FREQUENCY * 15.25);
    continent_def_tu0.set_power(CONTINENT_FREQUENCY / 113.75);
    continent_def_tu0.set_roughness(13);

    // 2: [Intermediate-turbulence module]: This turbulence module warps the
    //    output value from the coarse-turbulence module.  This turbulence has
    //    a higher frequency, but lower power, than the coarse-turbulence
    //    module, adding some intermediate detail to it.
    let mut continent_def_tu1 = Turbulence::new(continent_def_tu0.clone());
    continent_def_tu1.set_seed(seed + 11);
    continent_def_tu1.set_frequency(CONTINENT_FREQUENCY * 47.25);
    continent_def_tu1.set_power(CONTINENT_FREQUENCY / 433.75);
    continent_def_tu1.set_roughness(12);

    // 3: [Warped-base-continent-definition module]: This turbulence module
    //    warps the output value from the intermediate-turbulence module.  This
    //    turbulence has a higher frequency, but lower power, than the
    //    intermediate-turbulence module, adding some fine detail to it.
    let mut continent_def_tu2 = Turbulence::new(continent_def_tu1.clone());
    continent_def_tu2.set_seed(seed + 12);
    continent_def_tu2.set_frequency(CONTINENT_FREQUENCY * 95.25);
    continent_def_tu2.set_power(CONTINENT_FREQUENCY / 1019.75);
    continent_def_tu2.set_roughness(11);

    // 4: [Select-turbulence module]: At this stage, the turbulence is applied
    //    to the entire base-continent-definition subgroup, producing some very
    //    rugged, unrealistic coastlines.  This selector module selects the
    //    output values from the (unwarped) base-continent-definition subgroup
    //    and the warped-base-continent-definition module, based on the output
    //    value from the (unwarped) base-continent-definition subgroup.  The
    //    selection boundary is near sea level and has a relatively smooth
    //    transition.  In effect, only the higher areas of the base-continent-
    //    definition subgroup become warped; the underwater and coastal areas
    //    remain unaffected.
    let mut continent_def_se = Select::new(base_continent_def.clone(),
                                           continent_def_tu2.clone(),
                                           base_continent_def.clone());
    continent_def_se.set_bounds(SEA_LEVEL - 0.0375, SEA_LEVEL + 1000.0375);
    continent_def_se.set_edge_falloff(0.0625);

    // 7: [Continent-definition group]: Caches the output value from the
    //    clamped-continent module.  This is the output value for the entire
    //    continent-definition group.
    let continent_def: Rc<Module> = Rc::new(Cache::new(continent_def_se.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: terrain type definition
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: terrain type definition (3 noise modules)
    //
    // This subgroup defines the positions of the terrain types on the planet.
    //
    // Terrain types include, in order of increasing roughness, plains, hills,
    // and mountains.
    //
    // This subgroup's output value is based on the output value from the
    // continent-definition group.  Rougher terrain mainly appears at higher
    // elevations.
    //
    // -1.0 represents the smoothest terrain types (plains and underwater) and
    // +1.0 represents the roughest terrain types (mountains).
    //

    // 1: [Warped-continent module]: This turbulence module slightly warps the
    //    output value from the continent-definition group.  This prevents the
    //    rougher terrain from appearing exclusively at higher elevations.
    //    Rough areas may now appear in the the ocean, creating rocky islands
    //    and fjords.
    let mut terrain_type_def_tu = Turbulence::new(continent_def.clone());
    terrain_type_def_tu.set_seed(seed + 20);
    terrain_type_def_tu.set_frequency(CONTINENT_FREQUENCY * 18.125);
    terrain_type_def_tu.set_power(CONTINENT_FREQUENCY / 20.59375 * TERRAIN_OFFSET);
    terrain_type_def_tu.set_roughness(3);

    // 2: [Roughness-probability-shift module]: This terracing module sharpens
    //    the edges of the warped-continent module near sea level and lowers
    //    the slope towards the higher-elevation areas.  This shrinks the areas
    //    in which the rough terrain appears, increasing the "rarity" of rough
    //    terrain.
    let mut terrain_type_def_te = Terrace::new(terrain_type_def_tu.clone());
    terrain_type_def_te.add_control_point(-1.00);
    terrain_type_def_te.add_control_point(SHELF_LEVEL + SEA_LEVEL / 2.0);
    terrain_type_def_te.add_control_point(1.00);

    // 3: [Terrain-type-definition group]: Caches the output value from the
    //    roughness-probability-shift module.  This is the output value for
    //    the entire terrain-type-definition group.
    let terrain_type_def: Rc<Module> = Rc::new(Cache::new(terrain_type_def_te.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: mountainous terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: mountain base definition (9 noise modules)
    //
    // This subgroup generates the base-mountain elevations.  Other subgroups
    // will add the ridges and low areas to the base elevations.
    //
    // -1.0 represents low mountainous terrain and +1.0 represents high
    // mountainous terrain.
    //

    // 1: [Mountain-ridge module]: This ridged-multifractal-noise module
    //    generates the mountain ridges.
    let mut mountain_base_def_rm0 = RidgedMulti::new();
    mountain_base_def_rm0.set_seed(seed + 30);
    mountain_base_def_rm0.set_frequency(1723.0);
    mountain_base_def_rm0.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountain_base_def_rm0.set_octave_count(4);
    mountain_base_def_rm0.set_quality(NoiseQuality::Standard);

    // 2: [Scaled-mountain-ridge module]: Next, a scale/bias module scales the
    //    output value from the mountain-ridge module so that its ridges are not
    //    too high.  The reason for this is that another subgroup adds actual
    //    mountainous terrain to these ridges.
    let mut mountain_base_def_sb0 = ScaleBias::new(mountain_base_def_rm0.clone());
    mountain_base_def_sb0.set_scale(0.5);
    mountain_base_def_sb0.set_bias(0.375);

    // 3: [River-valley module]: This ridged-multifractal-noise module generates
    //    the river valleys.  It has a much lower frequency than the mountain-
    //    ridge module so that more mountain ridges will appear outside of the
    //    valleys.  Note that this noise module generates ridged-multifractal
    //    noise using only one octave; this information will be important in the
    //    next step.
    let mut mountain_base_def_rm1 = RidgedMulti::new();
    mountain_base_def_rm1.set_seed(seed + 31);
    mountain_base_def_rm1.set_frequency(367.0);
    mountain_base_def_rm1.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountain_base_def_rm1.set_octave_count(1);
    mountain_base_def_rm1.set_quality(NoiseQuality::Best);

    // 4: [Scaled-river-valley module]: Next, a scale/bias module applies a
    //    scaling factor of -2.0 to the output value from the river-valley
    //    module.  This stretches the possible elevation values because one-
    //    octave ridged-multifractal noise has a lower range of output values
    //    than multiple-octave ridged-multifractal noise.  The negative scaling
    //    factor inverts the range of the output value, turning the ridges from
    //    the river-valley module into valleys.
    let mut mountain_base_def_sb1 = ScaleBias::new(mountain_base_def_rm1.clone());
    mountain_base_def_sb1.set_scale(-2.0);
    mountain_base_def_sb1.set_bias(-0.5);

    // 5: [Low-flat module]: This low constant value is used by step 6.
    let mut mountain_base_def_co = Constant::new();
    mountain_base_def_co.set_const_value(-1.0);

    // 6: [Mountains-and-valleys module]: This blender module merges the
    //    scaled-mountain-ridge module and the scaled-river-valley module
    //    together.  It causes the low-lying areas of the terrain to become
    //    smooth, and causes the high-lying areas of the terrain to contain
    //    ridges.  To do this, it uses the scaled-river-valley module as the
    //    control module, causing the low-flat module to appear in the lower
    //    areas and causing the scaled-mountain-ridge module to appear in the
    //    higher areas.
    let mountain_base_def_bl = Blend::new(mountain_base_def_co.clone(),
                                          mountain_base_def_sb0.clone(),
                                          mountain_base_def_sb1.clone());

    // 7: [Coarse-turbulence module]: This turbulence module warps the output
    //    value from the mountain-and-valleys module, adding some coarse detail
    //    to it.
    let mut mountain_base_def_tu0 = Turbulence::new(mountain_base_def_bl.clone());
    mountain_base_def_tu0.set_seed(seed + 32);
    mountain_base_def_tu0.set_frequency(1337.0);
    mountain_base_def_tu0.set_power(1.0 / 6730.0 * MOUNTAINS_TWIST);
    mountain_base_def_tu0.set_roughness(4);

    // 8: [Warped-mountains-and-valleys module]: This turbulence module warps
    //    the output value from the coarse-turbulence module.  This turbulence
    //    has a higher frequency, but lower power, than the coarse-turbulence
    //    module, adding some fine detail to it.
    let mut mountain_base_def_tu1 = Turbulence::new(mountain_base_def_tu0.clone());
    mountain_base_def_tu1.set_seed(seed + 33);
    mountain_base_def_tu1.set_frequency(21221.0);
    mountain_base_def_tu1.set_power(1.0 / 120157.0 * MOUNTAINS_TWIST);
    mountain_base_def_tu1.set_roughness(6);

    // 9: [Mountain-base-definition subgroup]: Caches the output value from the
    //    warped-mountains-and-valleys module.
    let mountain_base_def: Rc<Module> = Rc::new(Cache::new(mountain_base_def_tu1.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: high mountainous terrain (5 noise modules)
    //
    // This subgroup generates the mountainous terrain that appears at high
    // elevations within the mountain ridges.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Mountain-basis-0 module]: This ridged-multifractal-noise module,
    //    along with the mountain-basis-1 module, generates the individual
    //    mountains.
    let mut mountainous_high_rm0 = RidgedMulti::new();
    mountainous_high_rm0.set_seed(seed + 40);
    mountainous_high_rm0.set_frequency(2371.0);
    mountainous_high_rm0.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountainous_high_rm0.set_octave_count(3);
    mountainous_high_rm0.set_quality(NoiseQuality::Best);

    // 2: [Mountain-basis-1 module]: This ridged-multifractal-noise module,
    //    along with the mountain-basis-0 module, generates the individual
    //    mountains.
    let mut mountainous_high_rm1 = RidgedMulti::new();
    mountainous_high_rm1.set_seed(seed + 41);
    mountainous_high_rm1.set_frequency(2341.0);
    mountainous_high_rm1.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountainous_high_rm1.set_octave_count(3);
    mountainous_high_rm1.set_quality(NoiseQuality::Best);

    // 3: [High-mountains module]: Next, a maximum-value module causes more
    //    mountains to appear at the expense of valleys.  It does this by
    //    ensuring that only the maximum of the output values from the two
    //    ridged-multifractal-noise modules contribute to the output value of
    //    this subgroup.
    let mountainous_high_ma = Max::new(mountainous_high_rm0.clone(), mountainous_high_rm1.clone());

    // 4: [Warped-high-mountains module]: This turbulence module warps the
    //    output value from the high-mountains module, adding some detail to it.
    let mut mountainous_high_tu = Turbulence::new(mountainous_high_ma.clone());
    mountainous_high_tu.set_seed(seed + 42);
    mountainous_high_tu.set_frequency(31511.0);
    mountainous_high_tu.set_power(1.0 / 180371.0 * MOUNTAINS_TWIST);
    mountainous_high_tu.set_roughness(4);

    // 5: [High-mountainous-terrain subgroup]: Caches the output value from the
    //    warped-high-mountains module.
    let mountainous_high: Rc<Module> = Rc::new(Cache::new(mountainous_high_tu.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: low mountainous terrain (4 noise modules)
    //
    // This subgroup generates the mountainous terrain that appears at low
    // elevations within the river valleys.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Lowland-basis-0 module]: This ridged-multifractal-noise module,
    //    along with the lowland-basis-1 module, produces the low mountainous
    //    terrain.
    let mut mountainous_low_rm0 = RidgedMulti::new();
    mountainous_low_rm0.set_seed(seed + 50);
    mountainous_low_rm0.set_frequency(1381.0);
    mountainous_low_rm0.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountainous_low_rm0.set_octave_count(8);
    mountainous_low_rm0.set_quality(NoiseQuality::Best);

    // 1: [Lowland-basis-1 module]: This ridged-multifractal-noise module,
    //    along with the lowland-basis-0 module, produces the low mountainous
    //    terrain.
    let mut mountainous_low_rm1 = RidgedMulti::new();
    mountainous_low_rm1.set_seed(seed + 51);
    mountainous_low_rm1.set_frequency(1427.0);
    mountainous_low_rm1.set_lacunarity(MOUNTAIN_LACUNARITY);
    mountainous_low_rm1.set_octave_count(8);
    mountainous_low_rm1.set_quality(NoiseQuality::Best);

    // 3: [Low-mountainous-terrain module]: This multiplication module combines
    //    the output values from the two ridged-multifractal-noise modules.
    //    This causes the following to appear in the resulting terrain:
    //    - Cracks appear when two negative output values are multiplied
    //      together.
    //    - Flat areas appear when a positive and a negative output value are
    //      multiplied together.
    //    - Ridges appear when two positive output values are multiplied
    //      together.
    let mountainous_low_mu = Multiply::new(mountainous_low_rm0.clone(),
                                           mountainous_low_rm1.clone());

    // 4: [Low-mountainous-terrain subgroup]: Caches the output value from the
    //    low-moutainous-terrain module.
    let mountainous_low: Rc<Module> = Rc::new(Cache::new(mountainous_low_mu.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: mountainous terrain (7 noise modules)
    //
    // This subgroup generates the final mountainous terrain by combining the
    // high-mountainous-terrain subgroup with the low-mountainous-terrain
    // subgroup.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Scaled-low-mountainous-terrain module]: First, this scale/bias module
    //    scales the output value from the low-mountainous-terrain subgroup to a
    //    very low value and biases it towards -1.0.  This results in the low
    //    mountainous areas becoming more-or-less flat with little variation.
    //    This will also result in the low mountainous areas appearing at the
    //    lowest elevations in this subgroup.
    let mut mountainous_terrain_sb0 = ScaleBias::new(mountainous_low.clone());
    mountainous_terrain_sb0.set_scale(0.03125);
    mountainous_terrain_sb0.set_bias(-0.96875);

    // 2: [Scaled-high-mountainous-terrain module]: Next, this scale/bias module
    //    scales the output value from the high-mountainous-terrain subgroup to
    //    1/4 of its initial value and biases it so that its output value is
    //    usually positive.
    let mut mountainous_terrain_sb1 = ScaleBias::new(mountainous_high.clone());
    mountainous_terrain_sb1.set_scale(0.25);
    mountainous_terrain_sb1.set_bias(0.25);

    // 3: [Added-high-mountainous-terrain module]: This addition module adds the
    //    output value from the scaled-high-mountainous-terrain module to the
    //    output value from the mountain-base-definition subgroup.  Mountains
    //    now appear all over the terrain.
    let mountainous_terrain_ad = Add::new(mountainous_terrain_sb1.clone(),
                                          mountain_base_def.clone());

    // 4: [Combined-mountainous-terrain module]: Note that at this point, the
    //    entire terrain is covered in high mountainous terrain, even at the low
    //    elevations.  To make sure the mountains only appear at the higher
    //    elevations, this selector module causes low mountainous terrain to
    //    appear at the low elevations (within the valleys) and the high
    //    mountainous terrain to appear at the high elevations (within the
    //    ridges.)  To do this, this noise module selects the output value from
    //    the added-high-mountainous-terrain module if the output value from the
    //    mountain-base-definition subgroup is higher than a set amount.
    //    Otherwise, this noise module selects the output value from the scaled-
    //    low-mountainous-terrain module.
    let mut mountainous_terrain_se = Select::new(mountainous_terrain_sb0.clone(),
                                                 mountainous_terrain_ad.clone(),
                                                 mountain_base_def.clone());
    mountainous_terrain_se.set_bounds(-0.5, 999.5);
    mountainous_terrain_se.set_edge_falloff(0.5);

    // 5: [Scaled-mountainous-terrain-module]: This scale/bias module slightly
    //    reduces the range of the output value from the combined-mountainous-
    //    terrain module, decreasing the heights of the mountain peaks.
    let mut mountainous_terrain_sb2 = ScaleBias::new(mountainous_terrain_se.clone());
    mountainous_terrain_sb2.set_scale(0.8);
    mountainous_terrain_sb2.set_bias(0.0);

    // 6: [Glaciated-mountainous-terrain-module]: This exponential-curve module
    //    applies an exponential curve to the output value from the scaled-
    //    mountainous-terrain module.  This causes the slope of the mountains to
    //    smoothly increase towards higher elevations, as if a glacier grinded
    //    out those mountains.  This exponential-curve module expects the output
    //    value to range from -1.0 to +1.0.
    let mut mountainous_terrain_ex = Exponent::new(mountainous_terrain_sb2.clone());
    mountainous_terrain_ex.set_exponent(MOUNTAIN_GLACIATION);

    // 7: [Mountainous-terrain group]: Caches the output value from the
    //    glaciated-mountainous-terrain module.  This is the output value for
    //    the entire mountainous-terrain group.
    let mountainous_terrain: Rc<Module> = Rc::new(Cache::new(mountainous_terrain_ex.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: hilly terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: hilly terrain (11 noise modules)
    //
    // This subgroup generates the hilly terrain.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Hills module]: This billow-noise module generates the hills.
    let mut hilly_terrain_bi = Billow::new();
    hilly_terrain_bi.set_seed(seed + 60);
    hilly_terrain_bi.set_frequency(1663.0);
    hilly_terrain_bi.set_persistence(0.5);
    hilly_terrain_bi.set_lacunarity(HILLS_LACUNARITY);
    hilly_terrain_bi.set_octave_count(6);
    hilly_terrain_bi.set_quality(NoiseQuality::Best);

    // 2: [Scaled-hills module]: Next, a scale/bias module scales the output
    //    value from the hills module so that its hilltops are not too high.
    //    The reason for this is that these hills are eventually added to the
    //    river valleys (see below.)
    let mut hilly_terrain_sb0 = ScaleBias::new(hilly_terrain_bi.clone());
    hilly_terrain_sb0.set_scale(0.5);
    hilly_terrain_sb0.set_bias(0.5);

    // 3: [River-valley module]: This ridged-multifractal-noise module generates
    //    the river valleys.  It has a much lower frequency so that more hills
    //    will appear in between the valleys.  Note that this noise module
    //    generates ridged-multifractal noise using only one octave; this
    //    information will be important in the next step.
    let mut hilly_terrain_rm = RidgedMulti::new();
    hilly_terrain_rm.set_seed(seed + 61);
    hilly_terrain_rm.set_frequency(367.5);
    hilly_terrain_rm.set_lacunarity(HILLS_LACUNARITY);
    hilly_terrain_rm.set_quality(NoiseQuality::Best);
    hilly_terrain_rm.set_octave_count(1);

    // 4: [Scaled-river-valley module]: Next, a scale/bias module applies a
    //    scaling factor of -2.0 to the output value from the river-valley
    //    module.  This stretches the possible elevation values because one-
    //    octave ridged-multifractal noise has a lower range of output values
    //    than multiple-octave ridged-multifractal noise.  The negative scaling
    //    factor inverts the range of the output value, turning the ridges from
    //    the river-valley module into valleys.
    let mut hilly_terrain_sb1 = ScaleBias::new(hilly_terrain_rm.clone());
    hilly_terrain_sb1.set_scale(-2.0);
    hilly_terrain_sb1.set_bias(-0.5);

    // 5: [Low-flat module]: This low constant value is used by step 6.
    let mut hilly_terrain_co = Constant::new();
    hilly_terrain_co.set_const_value(-1.0);

    // 6: [Mountains-and-valleys module]: This blender module merges the
    //    scaled-hills module and the scaled-river-valley module together.  It
    //    causes the low-lying areas of the terrain to become smooth, and causes
    //    the high-lying areas of the terrain to contain hills.  To do this, it
    //    uses the scaled-hills module as the control module, causing the low-
    //    flat module to appear in the lower areas and causing the scaled-river-
    //    valley module to appear in the higher areas.
    let hilly_terrain_bl = Blend::new(hilly_terrain_co.clone(),
                                      hilly_terrain_sb1.clone(),
                                      hilly_terrain_sb0.clone());

    // 7: [Scaled-hills-and-valleys module]: This scale/bias module slightly
    //    reduces the range of the output value from the hills-and-valleys
    //    module, decreasing the heights of the hilltops.
    let mut hilly_terrain_sb2 = ScaleBias::new(hilly_terrain_bl.clone());
    hilly_terrain_sb2.set_scale(0.75);
    hilly_terrain_sb2.set_bias(-0.25);

    // 8: [Increased-slope-hilly-terrain module]: To increase the hill slopes at
    //    higher elevations, this exponential-curve module applies an
    //    exponential curve to the output value the scaled-hills-and-valleys
    //    module.  This exponential-curve module expects the input value to
    //    range from -1.0 to 1.0.
    let mut hilly_terrain_ex = Exponent::new(hilly_terrain_sb2.clone());
    hilly_terrain_ex.set_exponent(1.375);

    // 9: [Coarse-turbulence module]: This turbulence module warps the output
    //    value from the increased-slope-hilly-terrain module, adding some
    //    coarse detail to it.
    let mut hilly_terrain_tu0 = Turbulence::new(hilly_terrain_ex.clone());
    hilly_terrain_tu0.set_seed(seed + 62);
    hilly_terrain_tu0.set_frequency(1531.0);
    hilly_terrain_tu0.set_power(1.0 / 16921.0 * HILLS_TWIST);
    hilly_terrain_tu0.set_roughness(4);

    // 10: [Warped-hilly-terrain module]: This turbulence module warps the
    //     output value from the coarse-turbulence module.  This turbulence has
    //     a higher frequency, but lower power, than the coarse-turbulence
    //     module, adding some fine detail to it.
    let mut hilly_terrain_tu1 = Turbulence::new(hilly_terrain_tu0.clone());
    hilly_terrain_tu1.set_seed(seed + 63);
    hilly_terrain_tu1.set_frequency(21617.0);
    hilly_terrain_tu1.set_power(1.0 / 117529.0 * HILLS_TWIST);
    hilly_terrain_tu1.set_roughness(6);

    // 11: [Hilly-terrain group]: Caches the output value from the warped-hilly-
    //     terrain module.  This is the output value for the entire hilly-
    //     terrain group.
    let hilly_terrain: Rc<Module> = Rc::new(Cache::new(hilly_terrain_tu1.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: plains terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: plains terrain (7 noise modules)
    //
    // This subgroup generates the plains terrain.
    //
    // Because this subgroup will eventually be flattened considerably, the
    // types and combinations of noise modules that generate the plains are not
    // really that important; they only need to "look" interesting.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Plains-basis-0 module]: This billow-noise module, along with the
    //    plains-basis-1 module, produces the plains.
    let mut plains_terrain_bi0 = Billow::new();
    plains_terrain_bi0.set_seed(seed + 70);
    plains_terrain_bi0.set_frequency(1097.5);
    plains_terrain_bi0.set_persistence(0.5);
    plains_terrain_bi0.set_lacunarity(PLAINS_LACUNARITY);
    plains_terrain_bi0.set_octave_count(8);
    plains_terrain_bi0.set_quality(NoiseQuality::Best);

    // 2: [Positive-plains-basis-0 module]: This scale/bias module makes the
    //    output value from the plains-basis-0 module positive since this output
    //    value will be multiplied together with the positive-plains-basis-1
    //    module.
    let mut plains_terrain_sb0 = ScaleBias::new(plains_terrain_bi0.clone());
    plains_terrain_sb0.set_scale(0.5);
    plains_terrain_sb0.set_bias(0.5);

    // 3: [Plains-basis-1 module]: This billow-noise module, along with the
    //    plains-basis-2 module, produces the plains.
    let mut plains_terrain_bi1 = Billow::new();
    plains_terrain_bi1.set_seed(seed + 71);
    plains_terrain_bi1.set_frequency(1319.5);
    plains_terrain_bi1.set_persistence(0.5);
    plains_terrain_bi1.set_lacunarity(PLAINS_LACUNARITY);
    plains_terrain_bi1.set_octave_count(8);
    plains_terrain_bi1.set_quality(NoiseQuality::Best);

    // 4: [Positive-plains-basis-1 module]: This scale/bias module makes the
    //    output value from the plains-basis-1 module positive since this output
    //    value will be multiplied together with the positive-plains-basis-0
    //    module.
    let mut plains_terrain_sb1 = ScaleBias::new(plains_terrain_bi1.clone());
    plains_terrain_sb1.set_scale(0.5);
    plains_terrain_sb1.set_bias(0.5);

    // 5: [Combined-plains-basis module]: This multiplication module combines
    //    the two plains basis modules together.
    let plains_terrain_mu = Multiply::new(plains_terrain_sb0.clone(), plains_terrain_sb1.clone());

    // 6: [Rescaled-plains-basis module]: This scale/bias module maps the output
    //    value that ranges from 0.0 to 1.0 back to a value that ranges from
    //    -1.0 to +1.0.
    let mut plains_terrain_sb2 = ScaleBias::new(plains_terrain_mu.clone());
    plains_terrain_sb2.set_scale(2.0);
    plains_terrain_sb2.set_bias(-1.0);

    // 7: [Plains-terrain group]: Caches the output value from the rescaled-
    //    plains-basis module.  This is the output value for the entire plains-
    //    terrain group.
    let plains_terrain: Rc<Module> = Rc::new(Cache::new(plains_terrain_sb2.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: badlands terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: badlands sand (6 noise modules)
    //
    // This subgroup generates the sandy terrain for the badlands.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Sand-dunes module]: This ridged-multifractal-noise module generates
    //    sand dunes.  This ridged-multifractal noise is generated with a single
    //    octave, which makes very smooth dunes.
    let mut badlands_sand_rm = RidgedMulti::new();
    badlands_sand_rm.set_seed(seed + 80);
    badlands_sand_rm.set_frequency(6163.5);
    badlands_sand_rm.set_lacunarity(BADLANDS_LACUNARITY);
    badlands_sand_rm.set_quality(NoiseQuality::Best);
    badlands_sand_rm.set_octave_count(1);

    // 2: [Scaled-sand-dunes module]: This scale/bias module shrinks the dune
    //    heights by a small amount.  This is necessary so that the subsequent
    //    noise modules in this subgroup can add some detail to the dunes.
    let mut badlands_sand_sb0 = ScaleBias::new(badlands_sand_rm.clone());
    badlands_sand_sb0.set_scale(0.875);
    badlands_sand_sb0.set_bias(0.0);

    // 3: [Dune-detail module]: This noise module uses Voronoi polygons to
    //    generate the detail to add to the dunes.  By enabling the distance
    //    algorithm, small polygonal pits are generated; the edges of the pits
    //    are joined to the edges of nearby pits.
    let mut badlands_sand_vo = Voronoi::new();
    badlands_sand_vo.set_seed(seed + 81);
    badlands_sand_vo.set_frequency(16183.25);
    badlands_sand_vo.set_displacement(0.0);
    badlands_sand_vo.enable_distance(true);

    // 4: [Scaled-dune-detail module]: This scale/bias module shrinks the dune
    //    details by a large amount.  This is necessary so that the subsequent
    //    noise modules in this subgroup can add this detail to the sand-dunes
    //    module.
    let mut badlands_sand_sb1 = ScaleBias::new(badlands_sand_vo.clone());
    badlands_sand_sb1.set_scale(0.25);
    badlands_sand_sb1.set_bias(0.25);

    // 5: [Dunes-with-detail module]: This addition module combines the scaled-
    //    sand-dunes module with the scaled-dune-detail module.
    let badlands_sand_ad = Add::new(badlands_sand_sb0.clone(), badlands_sand_sb1.clone());

    // 6: [Badlands-sand subgroup]: Caches the output value from the dunes-with-
    //    detail module.
    let badlands_sand: Rc<Module> = Rc::new(Cache::new(badlands_sand_ad.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: badlands cliffs (7 noise modules)
    //
    // This subgroup generates the cliffs for the badlands.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Cliff-basis module]: This Perlin-noise module generates some coherent
    //    noise that will be used to generate the cliffs.
    let mut badlands_cliffs_pe = Perlin::new();
    badlands_cliffs_pe.set_seed(seed + 90);
    badlands_cliffs_pe.set_frequency(CONTINENT_FREQUENCY * 839.0);
    badlands_cliffs_pe.set_persistence(0.5);
    badlands_cliffs_pe.set_lacunarity(BADLANDS_LACUNARITY);
    badlands_cliffs_pe.set_octave_count(6);
    badlands_cliffs_pe.set_quality(NoiseQuality::Standard);

    // 2: [Cliff-shaping module]: Next, this curve module applies a curve to the
    //    output value from the cliff-basis module.  This curve is initially
    //    very shallow, but then its slope increases sharply.  At the highest
    //    elevations, the curve becomes very flat again.  This produces the
    //    stereotypical Utah-style desert cliffs.
    let mut badlands_cliffs_cu = Curve::new(badlands_cliffs_pe.clone());
    badlands_cliffs_cu.add_control_point(-2.0000, -2.0000);
    badlands_cliffs_cu.add_control_point(-1.0000, -1.2500);
    badlands_cliffs_cu.add_control_point(-0.0000, -0.7500);
    badlands_cliffs_cu.add_control_point(0.5000, -0.2500);
    badlands_cliffs_cu.add_control_point(0.6250, 0.8750);
    badlands_cliffs_cu.add_control_point(0.7500, 1.0000);
    badlands_cliffs_cu.add_control_point(2.0000, 1.2500);

    // 3: [Clamped-cliffs module]: This clamping module makes the tops of the
    //    cliffs very flat by clamping the output value from the cliff-shaping
    //    module so that the tops of the cliffs are very flat.
    let mut badlands_cliffs_cl = Clamp::new(badlands_cliffs_cu.clone());
    badlands_cliffs_cl.set_bounds(-999.125, 0.875);

    // 4: [Terraced-cliffs module]: Next, this terracing module applies some
    //    terraces to the clamped-cliffs module in the lower elevations before
    //    the sharp cliff transition.
    let mut badlands_cliffs_te = Terrace::new(badlands_cliffs_cl.clone());
    badlands_cliffs_te.add_control_point(-1.0000);
    badlands_cliffs_te.add_control_point(-0.8750);
    badlands_cliffs_te.add_control_point(-0.7500);
    badlands_cliffs_te.add_control_point(-0.5000);
    badlands_cliffs_te.add_control_point(0.0000);
    badlands_cliffs_te.add_control_point(1.0000);

    // 5: [Coarse-turbulence module]: This turbulence module warps the output
    //    value from the terraced-cliffs module, adding some coarse detail to
    //    it.
    let mut badlands_cliffs_tu0 = Turbulence::new(badlands_cliffs_te.clone());
    badlands_cliffs_tu0.set_seed(seed + 91);
    badlands_cliffs_tu0.set_frequency(16111.0);
    badlands_cliffs_tu0.set_power(1.0 / 141539.0 * BADLANDS_TWIST);
    badlands_cliffs_tu0.set_roughness(3);

    // 6: [Warped-cliffs module]: This turbulence module warps the output value
    //    from the coarse-turbulence module.  This turbulence has a higher
    //    frequency, but lower power, than the coarse-turbulence module, adding
    //    some fine detail to it.
    let mut badlands_cliffs_tu1 = Turbulence::new(badlands_cliffs_tu0.clone());
    badlands_cliffs_tu1.set_seed(seed + 92);
    badlands_cliffs_tu1.set_frequency(36107.0);
    badlands_cliffs_tu1.set_power(1.0 / 211543.0 * BADLANDS_TWIST);
    badlands_cliffs_tu1.set_roughness(3);

    // 7: [Badlands-cliffs subgroup]: Caches the output value from the warped-
    //    cliffs module.
    let badlands_cliffs: Rc<Module> = Rc::new(Cache::new(badlands_cliffs_tu1.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: badlands terrain (3 noise modules)
    //
    // Generates the final badlands terrain.
    //
    // Using a scale/bias module, the badlands sand is flattened considerably,
    // then the sand elevations are lowered to around -1.0.  The maximum value
    // from the flattened sand module and the cliff module contributes to the
    // final elevation.  This causes sand to appear at the low elevations since
    // the sand is slightly higher than the cliff base.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Scaled-sand-dunes module]: This scale/bias module considerably
    //    flattens the output value from the badlands-sands subgroup and lowers
    //    this value to near -1.0.
    let mut badlands_terrain_sb = ScaleBias::new(badlands_sand.clone());
    badlands_terrain_sb.set_scale(0.25);
    badlands_terrain_sb.set_bias(-0.75);

    // 2: [Dunes-and-cliffs module]: This maximum-value module causes the dunes
    //    to appear in the low areas and the cliffs to appear in the high areas.
    //    It does this by selecting the maximum of the output values from the
    //    scaled-sand-dunes module and the badlands-cliffs subgroup.
    let badlands_terrain_ma = Max::new(badlands_cliffs.clone(), badlands_terrain_sb.clone());

    // 3: [Badlands-terrain group]: Caches the output value from the dunes-and-
    //    cliffs module.  This is the output value for the entire badlands-
    //    terrain group.
    let badlands_terrain: Rc<Module> = Rc::new(Cache::new(badlands_terrain_ma.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: river positions
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: river positions (7 noise modules)
    //
    // This subgroup generates the river positions.
    //
    // -1.0 represents the lowest elevations and +1.0 represents the highest
    // elevations.
    //

    // 1: [Large-river-basis module]: This ridged-multifractal-noise module
    //    creates the large, deep rivers.
    let mut river_positions_rm0 = RidgedMulti::new();
    river_positions_rm0.set_seed(seed + 100);
    river_positions_rm0.set_frequency(18.75);
    river_positions_rm0.set_lacunarity(CONTINENT_LACUNARITY);
    river_positions_rm0.set_octave_count(1);
    river_positions_rm0.set_quality(NoiseQuality::Best);

    // 2: [Large-river-curve module]: This curve module applies a curve to the
    //    output value from the large-river-basis module so that the ridges
    //    become inverted.  This creates the rivers.  This curve also compresses
    //    the edge of the rivers, producing a sharp transition from the land to
    //    the river bottom.
    let mut river_positions_cu0 = Curve::new(river_positions_rm0.clone());
    river_positions_cu0.add_control_point(-2.000, 2.000);
    river_positions_cu0.add_control_point(-1.000, 1.000);
    river_positions_cu0.add_control_point(-0.125, 0.875);
    river_positions_cu0.add_control_point(0.000, -1.000);
    river_positions_cu0.add_control_point(1.000, -1.500);
    river_positions_cu0.add_control_point(2.000, -2.000);

    /// 3: [Small-river-basis module]: This ridged-multifractal-noise module
    //     creates the small, shallow rivers.
    let mut river_positions_rm1 = RidgedMulti::new();
    river_positions_rm1.set_seed(seed + 101);
    river_positions_rm1.set_frequency(43.25);
    river_positions_rm1.set_lacunarity(CONTINENT_LACUNARITY);
    river_positions_rm1.set_octave_count(1);
    river_positions_rm1.set_quality(NoiseQuality::Best);

    // 4: [Small-river-curve module]: This curve module applies a curve to the
    //    output value from the small-river-basis module so that the ridges
    //    become inverted.  This creates the rivers.  This curve also compresses
    //    the edge of the rivers, producing a sharp transition from the land to
    //    the river bottom.
    let mut river_positions_cu1 = Curve::new(river_positions_rm1.clone());
    river_positions_cu1.add_control_point(-2.000, 2.0000);
    river_positions_cu1.add_control_point(-1.000, 1.5000);
    river_positions_cu1.add_control_point(-0.125, 1.4375);
    river_positions_cu1.add_control_point(0.000, 0.5000);
    river_positions_cu1.add_control_point(1.000, 0.2500);
    river_positions_cu1.add_control_point(2.000, 0.0000);

    // 5: [Combined-rivers module]: This minimum-value module causes the small
    //    rivers to cut into the large rivers.  It does this by selecting the
    //    minimum output values from the large-river-curve module and the small-
    //    river-curve module.
    let river_positions_mi = Min::new(river_positions_cu0.clone(), river_positions_cu1.clone());

    // 6: [Warped-rivers module]: This turbulence module warps the output value
    //    from the combined-rivers module, which twists the rivers.  The high
    //    roughness produces less-smooth rivers.
    let mut river_positions_tu = Turbulence::new(river_positions_mi.clone());
    river_positions_tu.set_seed(seed + 102);
    river_positions_tu.set_frequency(9.25);
    river_positions_tu.set_power(1.0 / 57.75);
    river_positions_tu.set_roughness(6);

    // 7: [River-positions group]: Caches the output value from the warped-
    //    rivers module.  This is the output value for the entire river-
    //    positions group.
    let river_positions: Rc<Module> = Rc::new(Cache::new(river_positions_tu.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: scaled mountainous terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: scaled mountainous terrain (6 noise modules)
    //
    // This subgroup scales the output value from the mountainous-terrain group
    // so that it can be added to the elevation defined by the continent-
    // definition group.
    //
    // This subgroup scales the output value such that it is almost always
    // positive.  This is done so that a negative elevation does not get applied
    // to the continent-definition group, preventing parts of that group from
    // having negative terrain features "stamped" into it.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Base-scaled-mountainous-terrain module]: This scale/bias module
    //    scales the output value from the mountainous-terrain group so that the
    //    output value is measured in planetary elevation units.
    let mut scaled_mountainous_terrain_sb0 = ScaleBias::new(mountainous_terrain.clone());
    scaled_mountainous_terrain_sb0.set_scale(0.125);
    scaled_mountainous_terrain_sb0.set_bias(0.125);

    // 2: [Base-peak-modulation module]: At this stage, most mountain peaks have
    //    roughly the same elevation.  This Perlin-noise module generates some
    //    random values that will be used by subsequent noise modules to
    //    randomly change the elevations of the mountain peaks.
    let mut scaled_mountainous_terrain_pe = Perlin::new();
    scaled_mountainous_terrain_pe.set_seed(seed + 110);
    scaled_mountainous_terrain_pe.set_frequency(14.5);
    scaled_mountainous_terrain_pe.set_persistence(0.5);
    scaled_mountainous_terrain_pe.set_lacunarity(MOUNTAIN_LACUNARITY);
    scaled_mountainous_terrain_pe.set_octave_count(6);
    scaled_mountainous_terrain_pe.set_quality(NoiseQuality::Standard);

    // 3: [Peak-modulation module]: This exponential-curve module applies an
    //    exponential curve to the output value from the base-peak-modulation
    //    module.  This produces a small number of high values and a much larger
    //    number of low values.  This means there will be a few peaks with much
    //    higher elevations than the majority of the peaks, making the terrain
    //    features more varied.
    let mut scaled_mountainous_terrain_ex = Exponent::new(scaled_mountainous_terrain_pe.clone());
    scaled_mountainous_terrain_ex.set_exponent(1.25);

    // 4: [Scaled-peak-modulation module]: This scale/bias module modifies the
    //    range of the output value from the peak-modulation module so that it
    //    can be used as the modulator for the peak-height-multiplier module.
    //    It is important that this output value is not much lower than 1.0.
    let mut scaled_mountainous_terrain_sb1 = ScaleBias::new(scaled_mountainous_terrain_ex.clone());
    scaled_mountainous_terrain_sb1.set_scale(0.25);
    scaled_mountainous_terrain_sb1.set_bias(1.0);

    // 5: [Peak-height-multiplier module]: This multiplier module modulates the
    //    heights of the mountain peaks from the base-scaled-mountainous-terrain
    //    module using the output value from the scaled-peak-modulation module.
    let scaled_mountainous_terrain_mu = Multiply::new(scaled_mountainous_terrain_sb0.clone(),
                                                      scaled_mountainous_terrain_sb1.clone());

    // 6: [Scaled-mountainous-terrain group]: Caches the output value from the
    //    peak-height-multiplier module.  This is the output value for the
    //    entire scaled-mountainous-terrain group.
    let scaled_mountainous_terrain: Rc<Module> =
        Rc::new(Cache::new(scaled_mountainous_terrain_mu.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: scaled hilly terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: scaled hilly terrain (6 noise modules)
    //
    // This subgroup scales the output value from the hilly-terrain group so
    // that it can be added to the elevation defined by the continent-
    // definition group.  The scaling amount applied to the hills is one half of
    // the scaling amount applied to the scaled-mountainous-terrain group.
    //
    // This subgroup scales the output value such that it is almost always
    // positive.  This is done so that negative elevations are not applied to
    // the continent-definition group, preventing parts of the continent-
    // definition group from having negative terrain features "stamped" into it.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Base-scaled-hilly-terrain module]: This scale/bias module scales the
    //    output value from the hilly-terrain group so that this output value is
    //    measured in planetary elevation units
    let mut scaled_hilly_terrain_sb0 = ScaleBias::new(hilly_terrain.clone());
    scaled_hilly_terrain_sb0.set_scale(0.0625);
    scaled_hilly_terrain_sb0.set_bias(0.0625);

    // 2: [Base-hilltop-modulation module]: At this stage, most hilltops have
    //    roughly the same elevation.  This Perlin-noise module generates some
    //    random values that will be used by subsequent noise modules to
    //    randomly change the elevations of the hilltops.
    let mut scaled_hilly_terrain_pe = Perlin::new();
    scaled_hilly_terrain_pe.set_seed(seed + 120);
    scaled_hilly_terrain_pe.set_frequency(13.5);
    scaled_hilly_terrain_pe.set_persistence(0.5);
    scaled_hilly_terrain_pe.set_lacunarity(HILLS_LACUNARITY);
    scaled_hilly_terrain_pe.set_octave_count(6);
    scaled_hilly_terrain_pe.set_quality(NoiseQuality::Standard);

    // 3: [Hilltop-modulation module]: This exponential-curve module applies an
    //    exponential curve to the output value from the base-hilltop-modulation
    //    module.  This produces a small number of high values and a much larger
    //    number of low values.  This means there will be a few hilltops with
    //    much higher elevations than the majority of the hilltops, making the
    //    terrain features more varied.
    let mut scaled_hilly_terrain_ex = Exponent::new(scaled_hilly_terrain_pe.clone());
    scaled_hilly_terrain_ex.set_exponent(1.25);

    // 4: [Scaled-hilltop-modulation module]: This scale/bias module modifies
    //    the range of the output value from the hilltop-modulation module so
    //    that it can be used as the modulator for the hilltop-height-multiplier
    //    module.  It is important that this output value is not much lower than
    //    1.0.
    let mut scaled_hilly_terrain_sb1 = ScaleBias::new(scaled_hilly_terrain_ex.clone());
    scaled_hilly_terrain_sb1.set_scale(0.5);
    scaled_hilly_terrain_sb1.set_bias(1.5);

    // 5: [Hilltop-height-multiplier module]: This multiplier module modulates
    //    the heights of the hilltops from the base-scaled-hilly-terrain module
    //    using the output value from the scaled-hilltop-modulation module.
    let scaled_hilly_terrain_mu = Multiply::new(scaled_hilly_terrain_sb0.clone(),
                                                scaled_hilly_terrain_sb1.clone());

    // 6: [Scaled-hilly-terrain group]: Caches the output value from the
    //    hilltop-height-multiplier module.  This is the output value for the
    //    entire scaled-hilly-terrain group.
    let scaled_hilly_terrain: Rc<Module> = Rc::new(Cache::new(scaled_hilly_terrain_mu.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: scaled plains terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: scaled plains terrain (2 noise modules)
    //
    // This subgroup scales the output value from the plains-terrain group so
    // that it can be added to the elevations defined by the continent-
    // definition group.
    //
    // This subgroup scales the output value such that it is almost always
    // positive.  This is done so that negative elevations are not applied to
    // the continent-definition group, preventing parts of the continent-
    // definition group from having negative terrain features "stamped" into it.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Scaled-plains-terrain module]: This scale/bias module greatly
    //    flattens the output value from the plains terrain.  This output value
    //    is measured in planetary elevation units
    let mut scaled_plains_terrain_sb = ScaleBias::new(plains_terrain.clone());
    scaled_plains_terrain_sb.set_scale(0.00390625);
    scaled_plains_terrain_sb.set_bias(0.0078125);

    // 2: [Scaled-plains-terrain group]: Caches the output value from the
    //    scaled-plains-terrain module.  This is the output value for the entire
    //    scaled-plains-terrain group.
    let scaled_plains_terrain: Rc<Module> = Rc::new(Cache::new(scaled_plains_terrain_sb.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: scaled badlands terrain
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: scaled badlands terrain (2 noise modules)
    //
    // This subgroup scales the output value from the badlands-terrain group so
    // that it can be added to the elevations defined by the continent-
    // definition group.
    //
    // This subgroup scales the output value such that it is almost always
    // positive.  This is done so that negative elevations are not applied to
    // the continent-definition group, preventing parts of the continent-
    // definition group from having negative terrain features "stamped" into it.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Scaled-badlands-terrain module]: This scale/bias module scales the
    //    output value from the badlands-terrain group so that it is measured
    //    in planetary elevation units
    let mut scaled_badlands_terrain_sb = ScaleBias::new(badlands_terrain.clone());
    scaled_badlands_terrain_sb.set_scale(0.0625);
    scaled_badlands_terrain_sb.set_bias(0.0625);

    // 2: [Scaled-badlands-terrain group]: Caches the output value from the
    //    scaled-badlands-terrain module.  This is the output value for the
    //    entire scaled-badlands-terrain group.
    let scaled_badlands_terrain: Rc<Module> =
        Rc::new(Cache::new(scaled_badlands_terrain_sb.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: final planet
    ////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continental shelf (6 noise modules)
    //
    // This module subgroup creates the continental shelves.
    //
    // The output value from this module subgroup are measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Shelf-creator module]: This terracing module applies a terracing
    //    curve to the continent-definition group at the specified shelf level.
    //    This terrace becomes the continental shelf.  Note that this terracing
    //    module also places another terrace below the continental shelf near
    //    -1.0.  The bottom of this terrace is defined as the bottom of the
    //    ocean; subsequent noise modules will later add oceanic trenches to the
    //    bottom of the ocean.
    let mut continental_shelf_te = Terrace::new(continent_def.clone());
    continental_shelf_te.add_control_point(-1.0);
    continental_shelf_te.add_control_point(-0.75);
    continental_shelf_te.add_control_point(SHELF_LEVEL);
    continental_shelf_te.add_control_point(1.0);

    // 2: [Oceanic-trench-basis module]: This ridged-multifractal-noise module
    //    generates some coherent noise that will be used to generate the
    //    oceanic trenches.  The ridges represent the bottom of the trenches.
    let mut continental_shelf_rm = RidgedMulti::new();
    continental_shelf_rm.set_seed(seed + 130);
    continental_shelf_rm.set_frequency(CONTINENT_FREQUENCY * 4.375);
    continental_shelf_rm.set_lacunarity(CONTINENT_LACUNARITY);
    continental_shelf_rm.set_octave_count(16);
    continental_shelf_rm.set_quality(NoiseQuality::Best);

    // 3: [Oceanic-trench module]: This scale/bias module inverts the ridges
    //    from the oceanic-trench-basis-module so that the ridges become
    //    trenches.  This noise module also reduces the depth of the trenches so
    //    that their depths are measured in planetary elevation units.
    let mut continental_shelf_sb = ScaleBias::new(continental_shelf_rm.clone());
    continental_shelf_sb.set_scale(-0.125);
    continental_shelf_sb.set_bias(-0.125);

    // 4: [Clamped-sea-bottom module]: This clamping module clamps the output
    //    value from the shelf-creator module so that its possible range is
    //    from the bottom of the ocean to sea level.  This is done because this
    //    subgroup is only concerned about the oceans.
    let mut continental_shelf_cl = Clamp::new(continental_shelf_te.clone());
    continental_shelf_cl.set_bounds(-0.75, SEA_LEVEL);

    // 5: [Shelf-and-trenches module]: This addition module adds the oceanic
    //    trenches to the clamped-sea-bottom module.
    let continental_shelf_ad = Add::new(continental_shelf_sb.clone(), continental_shelf_cl.clone());

    // 6: [Continental-shelf subgroup]: Caches the output value from the shelf-
    //    and-trenches module.
    let continental_shelf: Rc<Module> = Rc::new(Cache::new(continental_shelf_ad.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module group: base continent elevations (3 noise modules)
    //
    // This subgroup generates the base elevations for the continents, before
    // terrain features are added.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Base-scaled-continent-elevations module]: This scale/bias module
    //    scales the output value from the continent-definition group so that it
    //    is measured in planetary elevation units
    let mut base_continent_elev_sb = ScaleBias::new(continent_def.clone());
    base_continent_elev_sb.set_scale(CONTINENT_HEIGHT_SCALE);
    base_continent_elev_sb.set_bias(0.0);

    // 2: [Base-continent-with-oceans module]: This selector module applies the
    //    elevations of the continental shelves to the base elevations of the
    //    continent.  It does this by selecting the output value from the
    //    continental-shelf subgroup if the corresponding output value from the
    //    continent-definition group is below the shelf level.  Otherwise, it
    //    selects the output value from the base-scaled-continent-elevations
    //    module.
    let mut base_continent_elev_se = Select::new(base_continent_elev_sb.clone(),
                                                 continental_shelf.clone(),
                                                 continent_def.clone());
    base_continent_elev_se.set_bounds(SHELF_LEVEL - 1000.0, SHELF_LEVEL);
    base_continent_elev_se.set_edge_falloff(0.03125);

    // 3: [Base-continent-elevation subgroup]: Caches the output value from the
    //    base-continent-with-oceans module.
    let base_continent_elev: Rc<Module> = Rc::new(Cache::new(base_continent_elev_se.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continents with plains (2 noise modules)
    //
    // This subgroup applies the scaled-plains-terrain group to the base-
    // continent-elevation subgroup.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Continents-with-plains module]:  This addition module adds the
    //    scaled-plains-terrain group to the base-continent-elevation subgroup.
    let continents_with_plains_ad = Add::new(base_continent_elev.clone(),
                                             scaled_plains_terrain.clone());

    // 2: [Continents-with-plains subgroup]: Caches the output value from the
    //    continents-with-plains module.
    let continents_with_plains: Rc<Module> = Rc::new(Cache::new(continents_with_plains_ad.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continents with hills (3 noise modules)
    //
    // This subgroup applies the scaled-hilly-terrain group to the continents-
    // with-plains subgroup.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Continents-with-hills module]:  This addition module adds the scaled-
    //    hilly-terrain group to the base-continent-elevation subgroup.
    let continents_with_hills_ad = Add::new(base_continent_elev.clone(),
                                            scaled_hilly_terrain.clone());

    // 2: [Select-high-elevations module]: This selector module ensures that
    //    the hills only appear at higher elevations.  It does this by selecting
    //    the output value from the continent-with-hills module if the
    //    corresponding output value from the terrain-type-defintion group is
    //    above a certain value. Otherwise, it selects the output value from the
    //    continents-with-plains subgroup.
    let mut continents_with_hills_se = Select::new(continents_with_plains.clone(),
                                                   continents_with_hills_ad.clone(),
                                                   terrain_type_def.clone());
    continents_with_hills_se.set_bounds(1.0 - HILLS_AMOUNT, 1001.0 - HILLS_AMOUNT);
    continents_with_hills_se.set_edge_falloff(0.25);

    // 3: [Continents-with-hills subgroup]: Caches the output value from the
    //    select-high-elevations module.
    let continents_with_hills: Rc<Module> = Rc::new(Cache::new(continents_with_hills_se.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continents with mountains (5 noise modules)
    //
    // This subgroup applies the scaled-mountainous-terrain group to the
    // continents-with-hills subgroup.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Continents-and-mountains module]:  This addition module adds the
    //    scaled-mountainous-terrain group to the base-continent-elevation
    //    subgroup.
    let continents_with_mountains_ad0 = Add::new(base_continent_elev.clone(),
                                                 scaled_mountainous_terrain.clone());

    // 2: [Increase-mountain-heights module]:  This curve module applies a curve
    //    to the output value from the continent-definition group.  This
    //    modified output value is used by a subsequent noise module to add
    //    additional height to the mountains based on the current continent
    //    elevation.  The higher the continent elevation, the higher the
    //    mountains.
    let mut continents_with_mountains_cu = Curve::new(continent_def.clone());
    continents_with_mountains_cu.add_control_point(-1.0, -0.0625);
    continents_with_mountains_cu.add_control_point(0.0, 0.0000);
    continents_with_mountains_cu.add_control_point(1.0 - MOUNTAINS_AMOUNT, 0.0625);
    continents_with_mountains_cu.add_control_point(1.0, 0.2500);

    // 3: [Add-increased-mountain-heights module]: This addition module adds
    //    the increased-mountain-heights module to the continents-and-
    //    mountains module.  The highest continent elevations now have the
    //    highest mountains.
    let continents_with_mountains_ad1 = Add::new(continents_with_mountains_ad0.clone(),
                                                 continents_with_mountains_cu.clone());

    // 4: [Select-high-elevations module]: This selector module ensures that
    //    mountains only appear at higher elevations.  It does this by selecting
    //    the output value from the continent-with-mountains module if the
    //    corresponding output value from the terrain-type-defintion group is
    //    above a certain value.  Otherwise, it selects the output value from
    //    the continents-with-hills subgroup.  Note that the continents-with-
    //    hills subgroup also contains the plains terrain.
    let mut continents_with_mountains_se = Select::new(continents_with_hills.clone(),
                                                       continents_with_mountains_ad1.clone(),
                                                       terrain_type_def.clone());
    continents_with_mountains_se.set_bounds(1.0 - MOUNTAINS_AMOUNT, 1001.0 - MOUNTAINS_AMOUNT);
    continents_with_mountains_se.set_edge_falloff(0.25);

    // 5: [Continents-with-mountains subgroup]: Caches the output value from
    //    the select-high-elevations module.
    let continents_with_mountains: Rc<Module> =
        Rc::new(Cache::new(continents_with_mountains_se.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continents with badlands (5 noise modules)
    //
    // This subgroup applies the scaled-badlands-terrain group to the
    // continents-with-mountains subgroup.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Badlands-positions module]: This Perlin-noise module generates some
    //    random noise, which is used by subsequent noise modules to specify the
    //    locations of the badlands.
    let mut continents_with_badlands_pe = Perlin::new();
    continents_with_badlands_pe.set_seed(seed + 140);
    continents_with_badlands_pe.set_frequency(16.5);
    continents_with_badlands_pe.set_persistence(0.5);
    continents_with_badlands_pe.set_lacunarity(CONTINENT_LACUNARITY);
    continents_with_badlands_pe.set_octave_count(2);
    continents_with_badlands_pe.set_quality(NoiseQuality::Standard);

    // 2: [Continents-and-badlands module]:  This addition module adds the
    //    scaled-badlands-terrain group to the base-continent-elevation
    //    subgroup.
    let continents_with_badlands_ad = Add::new(base_continent_elev.clone(),
                                               scaled_badlands_terrain.clone());

    // 3: [Select-badlands-positions module]: This selector module places
    //    badlands at random spots on the continents based on the Perlin noise
    //    generated by the badlands-positions module.  To do this, it selects
    //    the output value from the continents-and-badlands module if the
    //    corresponding output value from the badlands-position module is
    //    greater than a specified value.  Otherwise, this selector module
    //    selects the output value from the continents-with-mountains subgroup.
    //    There is also a wide transition between these two noise modules so
    //    that the badlands can blend into the rest of the terrain on the
    //    continents.
    let mut continents_with_badlands_se = Select::new(continents_with_mountains.clone(),
                                                      continents_with_badlands_ad.clone(),
                                                      continents_with_badlands_pe.clone());
    continents_with_badlands_se.set_bounds(1.0 - BADLANDS_AMOUNT, 1001.0 - BADLANDS_AMOUNT);
    continents_with_badlands_se.set_edge_falloff(0.25);

    // 4: [Apply-badlands module]: This maximum-value module causes the badlands
    //    to "poke out" from the rest of the terrain.  It does this by ensuring
    //    that only the maximum of the output values from the continents-with-
    //    mountains subgroup and the select-badlands-positions modules
    //    contribute to the output value of this subgroup.  One side effect of
    //    this process is that the badlands will not appear in mountainous
    //    terrain.
    let continents_with_badlands_ma = Max::new(continents_with_mountains.clone(),
                                               continents_with_badlands_se.clone());

    // 5: [Continents-with-badlands subgroup]: Caches the output value from the
    //    apply-badlands module.
    let continents_with_badlands: Rc<Module> =
        Rc::new(Cache::new(continents_with_badlands_ma.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: continents with rivers (4 noise modules)
    //
    // This subgroup applies the river-positions group to the continents-with-
    // badlands subgroup.
    //
    // The output value from this module subgroup is measured in planetary
    // elevation units (-1.0 for the lowest underwater trenches and +1.0 for the
    // highest mountain peaks.)
    //

    // 1: [Scaled-rivers module]: This scale/bias module scales the output value
    //    from the river-positions group so that it is measured in planetary
    //    elevation units and is negative; this is required for step 2.
    let mut continents_with_rivers_sb = ScaleBias::new(river_positions.clone());
    continents_with_rivers_sb.set_scale(RIVER_DEPTH / 2.0);
    continents_with_rivers_sb.set_bias(-RIVER_DEPTH / 2.0);

    // 2: [Add-rivers-to-continents module]: This addition module adds the
    //    rivers to the continents-with-badlands subgroup.  Because the scaled-
    //    rivers module only outputs a negative value, the scaled-rivers module
    //    carves the rivers out of the terrain.
    let continents_with_rivers_ad = Add::new(continents_with_badlands.clone(),
                                             continents_with_rivers_sb.clone());

    // 3: [Blended-rivers-to-continents module]: This selector module outputs
    //    deep rivers near sea level and shallower rivers in higher terrain.  It
    //    does this by selecting the output value from the continents-with-
    //    badlands subgroup if the corresponding output value from the
    //    continents-with-badlands subgroup is far from sea level.  Otherwise,
    //    this selector module selects the output value from the add-rivers-to-
    //    continents module.
    let mut continents_with_rivers_se = Select::new(continents_with_badlands.clone(),
                                                    continents_with_rivers_ad.clone(),
                                                    continents_with_badlands.clone());
    continents_with_rivers_se.set_bounds(SEA_LEVEL, CONTINENT_HEIGHT_SCALE + SEA_LEVEL);
    continents_with_rivers_se.set_edge_falloff(CONTINENT_HEIGHT_SCALE - SEA_LEVEL);

    // 4: [Continents-with-rivers subgroup]: Caches the output value from the
    //    blended-rivers-to-continents module.
    let continents_with_rivers: Rc<Module> = Rc::new(Cache::new(continents_with_rivers_se.clone()));


    ////////////////////////////////////////////////////////////////////////////
    // Module subgroup: unscaled final planet (1 noise module)
    //
    // This subgroup simply caches the output value from the continent-with-
    // rivers subgroup to contribute to the final output value.
    //

    // 1: [Unscaled-final-planet subgroup]: Caches the output value from the
    //    continent-with-rivers subgroup.
    let unscaled_final_planet: Rc<Module> = Rc::new(Cache::new(continents_with_rivers.clone()));

    Box::new(unscaled_final_planet)
}

#[derive(Copy, Clone)]
enum Plane {
    XP,
    XN,
    YP,
    YN,
    ZP,
    ZN,
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Greyscale8,
    Greyscale16,
    Colour24,
}

fn lat_lon_to_pos(lat: f64, lon: f64) -> (f64, f64, f64) {
    let lat = lat.to_radians();
    let lon = lon.to_radians();
    let r = f64::cos(lat);
    let x = r * f64::cos(lon);
    let y = f64::sin(lat);
    let z = r * f64::sin(lon);
    (x, y, z)
}

pub fn clamp<T: Ord>(value: T, lower_bound: T, upper_bound: T) -> T {
    if value < lower_bound {
        lower_bound
    } else if value > upper_bound {
        upper_bound
    } else {
        value
    }
}

pub fn f64_clamp(value: f64, lower_bound: f64, upper_bound: f64) -> f64 {
    if value < lower_bound {
        lower_bound
    } else if value > upper_bound {
        upper_bound
    } else {
        value
    }
}

fn coord_to_pos(plane: Plane, a: usize, b: usize, max_coord: usize) -> (f64, f64, f64) {
    let (x, y, z) = match plane {
        Plane::XP => (max_coord, b, max_coord - a),
        Plane::XN => (0, b, a),
        Plane::YP => (a, max_coord, max_coord - b),
        Plane::YN => (a, 0, b),
        Plane::ZP => (a, b, max_coord),
        Plane::ZN => (max_coord - a, b, 0),
    };

    (-1.0 + x as f64 * 2.0 / max_coord as f64,
     -1.0 + y as f64 * 2.0 / max_coord as f64,
     -1.0 + z as f64 * 2.0 / max_coord as f64)
}

fn output_cube_face(plane: Plane,
                    seed: i32,
                    size: usize,
                    output_format: OutputFormat)
                    -> JoinHandle<()> {
    std::thread::spawn(move || {
        let generator = create_generator(seed);
        let mut dest_buffer: Vec<f64> = vec![0.0; size * size];

        for b in 0..size {
            let row_start = &mut dest_buffer[((size - 1 - b) * size)..];
            for a in 0..size {
                let (px, py, pz) = coord_to_pos(plane, a, b, size - 1);
                let magnitude = f64::sqrt(px * px + py * py + pz * pz);
                let px = px / magnitude;
                let py = py / magnitude;
                let pz = pz / magnitude;
                row_start[a] = generator.get_value(px, py, pz);
            }
        }

        let filename = match plane {
            Plane::XP => "xp.png",
            Plane::XN => "xn.png",
            Plane::YP => "yp.png",
            Plane::YN => "yn.png",
            Plane::ZP => "zp.png",
            Plane::ZN => "zn.png",
        };
        write_output_to_file(filename, &dest_buffer, size, size, output_format);
    })
}

fn output_cube(seed: i32, size: usize, output_format: OutputFormat) {
    let xp_join = output_cube_face(Plane::XP, seed, size, output_format);
    let xn_join = output_cube_face(Plane::XN, seed, size, output_format);
    let yp_join = output_cube_face(Plane::YP, seed, size, output_format);
    let yn_join = output_cube_face(Plane::YN, seed, size, output_format);
    let zp_join = output_cube_face(Plane::ZP, seed, size, output_format);
    let zn_join = output_cube_face(Plane::ZN, seed, size, output_format);

    xp_join.join().unwrap();
    xn_join.join().unwrap();
    yp_join.join().unwrap();
    yn_join.join().unwrap();
    zp_join.join().unwrap();
    zn_join.join().unwrap();
}

fn output_rect(seed: i32, width: usize, output_format: OutputFormat) {
    let height = width / 2;
    let generator = create_generator(seed);
    let mut dest_buffer: Vec<f64> = vec![0.0; width * height];

    for y in 0..height {
        let row_start = &mut dest_buffer[((height - 1 - y) * width)..];
        let cur_lat = -90.0 + (y as f64 / height as f64) * 180.0;
        for x in 0..width {
            let cur_lon = -180.0 + (x as f64 / width as f64) * 360.0;
            let pos = lat_lon_to_pos(cur_lat, cur_lon);
            row_start[x] = generator.get_value(pos.0, pos.1, pos.2);
        }
    }

    write_output_to_file("lat_lon.png", &dest_buffer, width, height, output_format);
}

fn write_output_to_file(filename: &str,
                        data: &[f64],
                        width: usize,
                        height: usize,
                        output_format: OutputFormat) {
    let img_data = match output_format {
        OutputFormat::Greyscale8 => {
            let mut img_data = Vec::new();
            img_data.resize(width * height, 0);
            let mut idx = 0;
            let mut img_idx = 0;
            for _ in 0..height {
                for _ in 0..width {
                    let value = (data[idx] + 1.0) / 2.0;
                    let value = (f64_clamp(value, 0.0, 1.0) * 255.0) as i32;
                    let value = clamp(value, 0, 0xff) as u8;
                    img_data[img_idx] = value;
                    idx += 1;
                    img_idx += 1;
                }
            }
            img_data
        }
        OutputFormat::Greyscale16 => {
            let mut img_data = Vec::new();
            img_data.resize(width * height * 2, 0);
            let mut idx = 0;
            let mut img_idx = 0;
            for _ in 0..height {
                for _ in 0..width {
                    let value = (data[idx] + 1.0) / 2.0;
                    let value = (f64_clamp(value, 0.0, 1.0) * 65535.0) as i32;
                    let value = clamp(value, 0, 0xffff);
                    img_data[img_idx] = ((value & 0xff00) >> 8) as u8;
                    img_data[img_idx + 1] = (value & 0x00ff) as u8;
                    idx += 1;
                    img_idx += 2;
                }
            }
            img_data
        }
        OutputFormat::Colour24 => {
            let mut img_data = Vec::new();
            img_data.resize(width * height * 3, 0);
            let mut idx = 0;
            let mut img_idx = 0;
            for _ in 0..height {
                for _ in 0..width {
                    let value = (data[idx] + 1.0) / 2.0;
                    let value = (f64_clamp(value, 0.0, 1.0) * 16777215.0) as i32;
                    let value = clamp(value, 0, 0xffffff);
                    let r = ((value & 0x00ff0000) >> 16) as u8;
                    let g = ((value & 0x0000ff00) >> 8) as u8;
                    let b = (value & 0x000000ff) as u8;
                    img_data[img_idx] = r;
                    img_data[img_idx + 1] = g;
                    img_data[img_idx + 2] = b;
                    idx += 1;
                    img_idx += 3;
                }
            }
            img_data
        }
    };

    let file = File::create(Path::new(filename)).expect("Failed to create file for writing");
    let writer = BufWriter::new(file);

    let ct = match output_format {
        OutputFormat::Greyscale8 => ColorType::Gray(8),
        OutputFormat::Greyscale16 => ColorType::Gray(16),
        OutputFormat::Colour24 => ColorType::RGB(8),
    };

    let encoder = PNGEncoder::new(writer);

    encoder.encode(&img_data, width as u32, height as u32, ct)
        .expect("Failed to encode image data");
}

fn main() {
    let matches = App::new("ComplexPlanet")
        .version(crate_version!())
        .about("Generate maps for a complex planetary surface. Based on the libnoise \
                complexplanet example")
        .arg(Arg::with_name("seed")
            .short("s")
            .long("seed")
            .default_value("0")
            .help("Specifies the seed to use to generate the planet, different seeds give \
                   different planets"))
        .arg(Arg::with_name("type")
            .long("type")
            .takes_value(true)
            .default_value("cube")
            .possible_value("cube")
            .possible_value("rect")
            .help("Specifies what format to output in"))
        .arg(Arg::with_name("width")
            .long("width")
            .default_value("1024")
            .help("Specifies the width of the images to generate"))
        .arg(Arg::with_name("format")
            .long("format")
            .default_value("greyscale8")
            .possible_value("greyscale8")
            .possible_value("greyscale16")
            .possible_value("colour24"))
        .get_matches();

    let seed = match i32::from_str(matches.value_of("seed").unwrap()) {
        Ok(seed) => seed,
        Err(_) => {
            println!("Seed must be an integer");
            std::process::exit(1);
        }
    };

    let width = match usize::from_str(matches.value_of("width").unwrap()) {
        Ok(seed) => seed,
        Err(_) => {
            println!("Width must be an integer");
            std::process::exit(1);
        }
    };

    let output_format = match matches.value_of("format").unwrap() {
        "greyscale8" => OutputFormat::Greyscale8,
        "greyscale16" => OutputFormat::Greyscale16,
        "colour24" => OutputFormat::Colour24,
        _ => unreachable!(),
    };

    match matches.value_of("type").unwrap() {
        "cube" => output_cube(seed, width, output_format),
        "rect" => output_rect(seed, width, output_format),
        _ => unreachable!(),
    }
}
