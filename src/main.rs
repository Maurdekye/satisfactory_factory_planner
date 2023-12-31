use clap::{ArgAction, Parser};
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    f32::consts::LN_2,
    fmt::Display,
    fs,
};

const BASIC_INGREDIENTS: [&str; 10] = [
    "Coal",
    "Limestone",
    "Iron Ore",
    "Copper Ore",
    "Bauxite",
    "Caterium Ore",
    "Raw Quartz",
    "Sulfur",
    "Crude Oil",
    "Water",
];

#[macro_export]
macro_rules! debug {
    ($val:expr) => {
        #[cfg(debug_assertions)]
        {
            dbg!($val);
        }
    };
}

trait DefaultDict<K, V>
where
    K: Eq + PartialEq + std::hash::Hash + Clone,
    V: Default,
{
    fn get_default(&mut self, key: &K) -> &mut V;
}

impl<K, V> DefaultDict<K, V> for HashMap<K, V>
where
    K: Eq + PartialEq + std::hash::Hash + Clone,
    V: Default,
{
    fn get_default(&mut self, key: &K) -> &mut V {
        self.entry(key.clone()).or_insert(V::default())
    }
}

#[derive(Debug, Clone)]
struct IndexedMap<K, V>
where
    K: std::hash::Hash + Eq + PartialEq,
{
    map: HashMap<K, Vec<V>>,
    index: HashMap<K, usize>,
}

impl<K, V> IndexedMap<K, V>
where
    K: std::hash::Hash + Eq + PartialEq,
{
    fn new() -> IndexedMap<K, V> {
        IndexedMap {
            map: HashMap::new(),
            index: HashMap::new(),
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|value_list| {
            value_list
                .get(
                    *self
                        .index
                        .get(key)
                        .unwrap_or(&0)
                        .clamp(&0, &(value_list.len() - 1)),
                )
                .unwrap()
        })
    }
}

impl<K, V, I> From<I> for IndexedMap<K, V>
where
    K: std::hash::Hash + Eq + PartialEq + Clone,
    I: Iterator<Item = (K, V)>,
{
    fn from(value: I) -> Self {
        let mut map = IndexedMap::new();
        for (key, val) in value {
            map.map.get_default(&key).push(val);
        }
        map
    }
}

#[derive(Deserialize, Clone)]
struct Recipe {
    machine: String,
    ingredients: Vec<(String, f32)>,
    products: Vec<(String, f32)>,
}

#[derive(Clone, Debug)]
enum Source {
    Recipe {
        machine: String,
        machine_quantity: f32,
        byproducts: Vec<(String, f32)>,
        ingredients: Vec<Product>,
    },
    Supply,
    Byproduct,
}

#[derive(Clone, Debug)]
struct Product {
    name: String,
    unsupplied: f32,
    sources: Vec<(f32, Source)>,
}

impl Product {
    fn adjust_quantities(&mut self, adjustment: f32) {
        self.unsupplied *= adjustment;
        for (ref mut quantity, ref mut source) in self.sources.iter_mut() {
            *quantity *= adjustment;
            match source {
                Source::Recipe {
                    machine_quantity,
                    ingredients,
                    byproducts,
                    ..
                } => {
                    *machine_quantity *= adjustment;
                    ingredients
                        .iter_mut()
                        .for_each(|ingredient| ingredient.adjust_quantities(adjustment));
                    byproducts
                        .iter_mut()
                        .for_each(|(_, byproduct_quantity)| *byproduct_quantity *= adjustment);
                }
                _ => (),
            }
        }
    }
}

impl Display for Product {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ProductDisplay {
            product: self.clone(),
            indent: 0,
        }
        .fmt(f)
    }
}

struct ProductDisplay {
    product: Product,
    indent: usize,
}

impl Display for ProductDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (quantity, source) in &self.product.sources {
            match source {
                Source::Recipe {
                    machine,
                    machine_quantity,
                    ingredients,
                    byproducts,
                } => {
                    writeln!(
                        f,
                        "{:>indent$} * {:.2} {}: {:.2} {}",
                        "",
                        quantity,
                        self.product.name,
                        machine_quantity,
                        machine,
                        indent = self.indent,
                    )?;
                    for sub_product in ingredients.iter() {
                        ProductDisplay {
                            product: sub_product.clone(),
                            indent: self.indent + 2,
                        }
                        .fmt(f)?;
                    }
                    for (byproduct, byproduct_quantity) in byproducts.iter() {
                        writeln!(
                            f,
                            "{:>indent$} < {:.2} {}",
                            "",
                            byproduct_quantity,
                            byproduct,
                            indent = self.indent,
                        )?;
                    }
                }
                Source::Supply => {
                    writeln!(
                        f,
                        "{:>indent$} - {:.2} {}",
                        "",
                        quantity,
                        self.product.name,
                        indent = self.indent,
                    )?;
                }
                Source::Byproduct => {
                    writeln!(
                        f,
                        "{:>indent$} > {:.2} {}",
                        "",
                        quantity,
                        self.product.name,
                        indent = self.indent,
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DependencyResolutionTotals {
    inputs: HashMap<String, f32>,
    byproduct_inputs: HashMap<String, f32>,
    intermediate_ingredients: HashMap<String, f32>,
    outputs: HashMap<String, f32>,
    byproducts: HashMap<String, f32>,
    machines: HashMap<String, HashMap<String, f32>>,
}

impl DependencyResolutionTotals {
    fn new() -> DependencyResolutionTotals {
        DependencyResolutionTotals {
            inputs: HashMap::new(),
            byproduct_inputs: HashMap::new(),
            intermediate_ingredients: HashMap::new(),
            outputs: HashMap::new(),
            byproducts: HashMap::new(),
            machines: HashMap::new(),
        }
    }

    fn from(dependency_trees: &Vec<Product>) -> DependencyResolutionTotals {
        let mut totals = DependencyResolutionTotals::new();
        totals.tally_trees(dependency_trees);
        totals
    }

    fn tally_trees(&mut self, dependency_trees: &Vec<Product>) {
        dependency_trees.iter().for_each(|product| {
            // tally outputs
            for (quantity, _) in &product.sources {
                *self.outputs.get_default(&product.name) += quantity;
            }

            // tally sub-nodes
            self.tally_node(product);
        })
    }

    fn tally_node(&mut self, node: &Product) {
        for (_, source) in &node.sources {
            match source {
                Source::Recipe {
                    machine,
                    machine_quantity,
                    ingredients,
                    byproducts,
                } => {
                    // tally machine counts
                    *self.machines.get_default(&machine).get_default(&node.name) +=
                        machine_quantity;

                    // tally byproducts
                    byproducts.iter().for_each(|(product, quantity)| {
                        *self.byproducts.get_default(&product) += quantity
                    });

                    // tally intermediate ingredients, inputs, and sub-nodes
                    ingredients.iter().for_each(|product| {
                        for (quantity, sub_source) in &product.sources {
                            match sub_source {
                                Source::Recipe { .. } => {
                                    *self.intermediate_ingredients.get_default(&product.name) +=
                                        quantity;
                                    self.tally_node(product);
                                }
                                Source::Supply => {
                                    *self.inputs.get_default(&product.name) += quantity;
                                }
                                Source::Byproduct => {
                                    *self.byproduct_inputs.get_default(&product.name) += quantity;
                                }
                            }
                        }
                    });
                }
                _ => (),
            }
        }
    }
}

impl Display for DependencyResolutionTotals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        DependencyResolutionTotalsDisplay {
            totals: self,
            show_perfect_splits: false,
        }
        .fmt(f)
    }
}

struct DependencyResolutionTotalsDisplay<'a> {
    totals: &'a DependencyResolutionTotals,
    show_perfect_splits: bool,
}

impl Display for DependencyResolutionTotalsDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let unused_byproducts = self
            .totals
            .byproducts
            .iter()
            .filter_map(|(byproduct, quantity_produced)| {
                let quantity_used = self.totals.byproduct_inputs.get(byproduct).unwrap_or(&0.0);
                if quantity_used >= quantity_produced {
                    None
                } else {
                    Some((byproduct.clone(), quantity_produced - quantity_used))
                }
            })
            .collect();

        for (heading, product_list) in vec![
            ("Input Ingredients:", &self.totals.inputs),
            (
                "Intermediate Ingredients:",
                &self.totals.intermediate_ingredients,
            ),
            ("Output Products:", &self.totals.outputs),
            ("Byproducts:", &unused_byproducts),
        ] {
            if !product_list.is_empty() {
                writeln!(f, "{heading}")?;
                for (product, quantity) in product_list.iter() {
                    writeln!(f, " * {:.2} {}", quantity, product)?;
                }

                writeln!(f)?;
            }
        }

        if !self.totals.byproducts.is_empty() {}

        writeln!(f, "Machines:")?;
        for (machine, machine_products) in self.totals.machines.iter() {
            writeln!(f, " * {}", machine)?;
            for (product, quantity) in machine_products.iter() {
                if self.show_perfect_splits {
                    let round_up = quantity.ceil() as u32;
                    let (splitters_2, splitters_3, perfect_split_quantity) =
                        nearest_perfect_split(round_up).unwrap();
                    writeln!(
                        f,
                        "   - {:.2} for {}s, or 2^{} * 3^{} = {} at {:.2}%",
                        quantity,
                        product,
                        splitters_2,
                        splitters_3,
                        perfect_split_quantity,
                        (quantity / perfect_split_quantity as f32) * 100.0
                    )?;
                } else {
                    writeln!(f, "   - {:.2} for {}s", quantity, product)?;
                }
            }
        }

        Ok(())
    }
}

const LN_3: f32 = 1.0986122886681098f32;
const FRAC_LN_2_LN_3: f32 = LN_2 / LN_3;
fn uceil(x: f32) -> u32 {
    x.ceil() as u32
}

/// this algorithm is an unholy abomination
fn nearest_perfect_split(base_machine_count: u32) -> Option<(u32, u32, u32)> {
    let ln_c: f32 = (base_machine_count as f32).ln();
    let b: f32 = ln_c / LN_3;
    let f = |x: u32| uceil((-FRAC_LN_2_LN_3 * (x as f32) + b).max(0.0));
    let mut result: Option<(u32, u32, u32)> = None;
    let mut closest_dist: Option<u32> = None;
    let mut x_pow2: u32 = 1;
    let mut last_y: u32 = f(0);
    let mut y_pow3: u32 = 3u32.pow(last_y);
    for x in 0..=uceil(ln_c / LN_2) {
        let y: u32 = f(x);
        if y != last_y {
            if y == last_y - 1 {
                y_pow3 /= 3;
            } else {
                y_pow3 = 3u32.pow(y);
            }
            last_y = y;
        }
        let split_value: u32 = x_pow2 * y_pow3;
        let new_dist: u32 = split_value - base_machine_count;
        if closest_dist.map_or(true, |c_dist| new_dist < c_dist) {
            result = Some((x, y, split_value));
            if new_dist == 0 {
                break;
            }
            closest_dist = Some(new_dist);
        }
        x_pow2 <<= 1;
    }
    result
}

fn resolve_product_dependencies(
    recipes: &IndexedMap<String, Recipe>,
    product: &mut Product,
    available_ingredients: &Vec<String>,
    available_byproducts: &HashMap<String, f32>,
) {
    // iterate down the tree to hit every node
    for (_, source) in product.sources.iter_mut() {
        match source {
            Source::Recipe {
                ingredients,
                byproducts,
                ..
            } => {
                let byproducts_map = byproducts.clone().into_iter().collect::<HashMap<_, _>>();
                for ingredient in ingredients.iter_mut() {
                    resolve_product_dependencies(
                        recipes,
                        ingredient,
                        available_ingredients,
                        &available_byproducts
                            .iter()
                            .map(|(byproduct, quantity)| {
                                (
                                    byproduct.clone(),
                                    quantity - byproducts_map.get(byproduct).unwrap_or(&0.0),
                                )
                            })
                            .collect(),
                    )
                }
            }
            _ => (),
        }
    }

    // cater to unsupplied required resources
    if product.unsupplied > 0.0 {
        if BASIC_INGREDIENTS.contains(&product.name.as_str())
            || available_ingredients.contains(&product.name)
        {
            product.sources.push((product.unsupplied, Source::Supply));
        } else if available_byproducts
            .get(&product.name)
            .map_or(false, |quantity| *quantity > 0.0)
        {
            product
                .sources
                .push((product.unsupplied, Source::Byproduct));
        } else {
            match recipes.get(&product.name) {
                None => product.sources.push((product.unsupplied, Source::Supply)),
                Some(recipe) => {
                    if product.unsupplied > 0.0 {
                        // determine production ratio
                        let production_ratio = product.unsupplied
                            / recipe
                                .products
                                .iter()
                                .find(|(recipe_product, _)| *recipe_product == product.name)
                                .expect("Recipe in value missing product from its key?!")
                                .1;

                        // compute ingredient dependencies
                        let byproducts = recipe
                            .products
                            .iter()
                            .filter_map(|(recipe_product, quantity)| {
                                if *recipe_product != product.name {
                                    Some((recipe_product.clone(), quantity * production_ratio))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<(String, f32)>>();
                        let byproducts_map =
                            byproducts.clone().into_iter().collect::<HashMap<_, _>>();

                        product.sources.push((
                            product.unsupplied,
                            Source::Recipe {
                                machine: recipe.machine.clone(),
                                machine_quantity: production_ratio,

                                byproducts,
                                ingredients: recipe
                                    .ingredients
                                    .iter()
                                    .map(|(recipe_product, quantity)| {
                                        let mut inner_product = Product {
                                            name: recipe_product.clone(),
                                            unsupplied: quantity * production_ratio,
                                            sources: Vec::new(),
                                        };
                                        resolve_product_dependencies(
                                            recipes,
                                            &mut inner_product,
                                            available_ingredients,
                                            &available_byproducts
                                                .iter()
                                                .map(|(byproduct, quantity)| {
                                                    (
                                                        byproduct.clone(),
                                                        quantity
                                                            - byproducts_map
                                                                .get(byproduct)
                                                                .unwrap_or(&0.0),
                                                    )
                                                })
                                                .collect(),
                                        );
                                        inner_product
                                    })
                                    .collect(),
                            },
                        ));
                    }
                }
            };
        }
    }
    product.unsupplied = 0.0;
}

fn apply_insufficient_supply_proportions(
    recipes: &IndexedMap<String, Recipe>,
    product: &mut Product,
    resupply_proportions: &HashMap<String, f32>,
) {
    for (quantity, source) in product.sources.iter_mut() {
        match source {
            Source::Recipe { ingredients, .. } => {
                for ingredient in ingredients.iter_mut() {
                    apply_insufficient_supply_proportions(
                        recipes,
                        ingredient,
                        resupply_proportions,
                    );
                }
            }
            _ => {
                resupply_proportions.get(&product.name).map(|proportion| {
                    let new_quantity = *quantity * proportion;
                    product.unsupplied = *quantity - new_quantity;
                    *quantity = new_quantity;
                });
            }
        }
    }
    product.sources.retain(|(quantity, _)| *quantity > 0.0);
}

fn compute_supply_proportions(
    inputs: &HashMap<String, f32>,
    ingredients: &HashMap<String, Option<f32>>,
) -> Vec<(String, f32)> {
    ingredients
        .iter()
        .filter_map(
            |(ingredient, maybe_quantity)| match inputs.get(ingredient) {
                Some(ingredient_quantity) => maybe_quantity.map(|available_ingredient_quantity| {
                    (
                        ingredient.clone(),
                        available_ingredient_quantity / ingredient_quantity,
                    )
                }),
                None => None,
            },
        )
        .collect()
}

fn resolve_dependency_trees(
    recipes: &IndexedMap<String, Recipe>,
    products: Vec<(String, Option<f32>)>,
    ingredients: Vec<(String, Option<f32>)>,
    resupply_insufficient: bool,
    reuse_byproducts: bool,
) -> (Vec<Product>, DependencyResolutionTotals) {
    let mut ingredients = ingredients.into_iter().collect::<HashMap<_, _>>();
    debug!(&ingredients);

    let mut initial_byproducts = HashMap::new();

    loop {
        let mut input_byproducts = initial_byproducts.clone();
        debug!(&input_byproducts);

        // fetch list of requests with specific quantities
        let quantity_requested_trees = {
            let mut trees = products
                .iter()
                .filter_map(|(name, maybe_quantity)| {
                    maybe_quantity.map(|quantity| Product {
                        name: name.clone(),
                        unsupplied: quantity,
                        sources: Vec::new(),
                    })
                })
                .collect::<Vec<_>>();
            debug!(&trees);

            if !trees.is_empty() {
                let ingredient_set = ingredients.iter().map(|(i, _)| i.clone()).collect();
                debug!(&ingredient_set);

                for tree in &mut trees {
                    resolve_product_dependencies(recipes, tree, &ingredient_set, &input_byproducts);
                }
                debug!(&trees);

                let mut totals = DependencyResolutionTotals::from(&trees);
                debug!(&totals);

                let initial_supply_proportions =
                    compute_supply_proportions(&totals.inputs, &ingredients);
                debug!(&initial_supply_proportions);

                let mut insufficient_ingredients = initial_supply_proportions
                    .clone()
                    .into_iter()
                    .filter(|(_, proportion)| *proportion < 1.0)
                    .collect::<HashMap<_, _>>();
                debug!(&insufficient_ingredients);

                let byproduct_some_set = input_byproducts
                    .clone()
                    .into_iter()
                    .map(|(byproduct, quantity)| (byproduct, Some(quantity)))
                    .collect();

                let mut insufficient_byproduct_inputs =
                    compute_supply_proportions(&totals.byproduct_inputs, &byproduct_some_set)
                        .into_iter()
                        .filter(|(_, proportion)| *proportion < 1.0)
                        .collect::<HashMap<_, _>>();
                debug!(&insufficient_byproduct_inputs);

                // adjust output if some provided quantities are insufficient
                while !insufficient_ingredients.is_empty()
                    || !insufficient_byproduct_inputs.is_empty()
                {
                    if !insufficient_ingredients.is_empty() {
                        if resupply_insufficient {
                            // resupply insufficient supplies
                            let ingredient_set_sans_resupplies = ingredients
                                .iter()
                                .filter(|(ingredient, _)| {
                                    !insufficient_ingredients.contains_key(*ingredient)
                                })
                                .map(|(ingredient, _)| ingredient.clone())
                                .collect();
                            debug!(&ingredient_set_sans_resupplies);

                            for tree in &mut trees {
                                apply_insufficient_supply_proportions(
                                    recipes,
                                    tree,
                                    &insufficient_ingredients,
                                );
                                debug!(&tree);
                                resolve_product_dependencies(
                                    recipes,
                                    tree,
                                    &ingredient_set_sans_resupplies,
                                    &input_byproducts,
                                );
                            }
                            debug!(&trees);
                        } else {
                            // adjust output to accommodate for lowest supplied ingredient
                            let lowest_supply = initial_supply_proportions
                                .iter()
                                .map(|(_, quantity)| *quantity)
                                .min_by(|a, b| {
                                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                                });
                            debug!(&lowest_supply);

                            lowest_supply.map(|supply| {
                                for tree in &mut trees {
                                    tree.adjust_quantities(supply);
                                }
                                debug!(&trees);
                            });
                        }
                    }

                    if !insufficient_byproduct_inputs.is_empty() {
                        for tree in &mut trees {
                            apply_insufficient_supply_proportions(
                                recipes,
                                tree,
                                &insufficient_byproduct_inputs,
                            );
                            debug!(&tree);
                            resolve_product_dependencies(
                                recipes,
                                tree,
                                &ingredient_set,
                                &HashMap::new(),
                            );
                        }
                        debug!(&trees);
                    }

                    totals = DependencyResolutionTotals::from(&trees);
                    debug!(&totals);

                    insufficient_ingredients =
                        compute_supply_proportions(&totals.inputs, &ingredients)
                            .into_iter()
                            .filter(|(_, proportion)| *proportion < 1.0)
                            .collect::<HashMap<_, _>>();
                    debug!(&insufficient_ingredients);

                    insufficient_byproduct_inputs =
                        compute_supply_proportions(&totals.byproduct_inputs, &byproduct_some_set)
                            .into_iter()
                            .filter(|(_, proportion)| *proportion < 1.0)
                            .collect::<HashMap<_, _>>();
                    debug!(&insufficient_byproduct_inputs);
                }

                // adjust available ingredients
                for (ingredient, used_quantity) in totals.inputs.iter() {
                    if ingredients.contains_key(ingredient) {
                        ingredients.entry(ingredient.clone()).and_modify(|entry| {
                            *entry = entry.map(|available_quantity| {
                                (available_quantity - used_quantity).max(0.0)
                            })
                        });
                    }
                }
                debug!(&ingredients);

                // adjust available byproducts
                for (byproduct, used_quantity) in totals.byproduct_inputs.iter() {
                    if input_byproducts.contains_key(byproduct) {
                        input_byproducts
                            .entry(byproduct.clone())
                            .and_modify(|entry| {
                                *entry = (*entry - used_quantity).max(0.0);
                            });
                    }
                }
                debug!(&input_byproducts);
            }
            trees
        };

        // fetch list of resources without specified quantities
        let quantity_unrequested_trees = {
            let mut trees = products
                .iter()
                .filter_map(|(name, maybe_quantity)| match maybe_quantity {
                    None => Some(Product {
                        name: name.clone(),
                        unsupplied: recipes
                            .get(name)
                            .map(|recipe| {
                                recipe
                                    .products
                                    .iter()
                                    .find(|(p, _)| p == name)
                                    .map(|(_, q)| q.clone())
                            })
                            .unwrap_or(Some(1.0))
                            .unwrap_or(1.0),
                        sources: Vec::new(),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            debug!(&trees);

            if !trees.is_empty() {
                let ingredient_set = ingredients.iter().map(|(i, _)| i.clone()).collect();
                debug!(&ingredient_set);

                for tree in &mut trees {
                    resolve_product_dependencies(recipes, tree, &ingredient_set, &input_byproducts);
                }
                debug!(&trees);

                let totals = DependencyResolutionTotals::from(&trees);
                debug!(&totals);

                let byproduct_some_set = input_byproducts
                    .clone()
                    .into_iter()
                    .map(|(byproduct, quantity)| (byproduct, Some(quantity)))
                    .collect();

                let mut supply_proportions: HashMap<String, f32> = HashMap::new();
                compute_supply_proportions(&totals.inputs, &ingredients)
                    .into_iter()
                    .chain(
                        compute_supply_proportions(&totals.byproduct_inputs, &byproduct_some_set)
                            .into_iter(),
                    )
                    .for_each(|(input, quantity)| {
                        *supply_proportions.get_default(&input) += quantity;
                    });
                debug!(&supply_proportions);

                // adjust output to acommodate for lowest supplied ingredient
                let lowest_supply = supply_proportions
                    .iter()
                    .map(|(_, quantity)| *quantity)
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                debug!(&lowest_supply);

                lowest_supply.map(|supply| {
                    for tree in &mut trees {
                        tree.adjust_quantities(supply);
                    }
                    debug!(&trees);
                });
            }
            trees
        };

        let trees: Vec<Product> = quantity_requested_trees
            .into_iter()
            .chain(quantity_unrequested_trees.into_iter())
            .collect();
        debug!(&trees);

        let totals = DependencyResolutionTotals::from(&trees);
        debug!(&totals);

        debug!(&initial_byproducts);
        if reuse_byproducts && totals.byproducts != initial_byproducts {
            initial_byproducts = totals.byproducts;
        } else {
            return (trees, totals);
        }
    }
}

fn find_product_name(products: &HashSet<String>, name: &String) -> String {
    let name = name.trim().to_lowercase();
    products
        .iter()
        .find(|full_name| full_name.to_lowercase() == name)
        .unwrap_or(&name)
        .clone()
}

fn parse_product_list(products: &HashSet<String>, raw: &String) -> Vec<(String, Option<f32>)> {
    let part_pattern = Regex::new(r"^([^:]*)(:\s*(\d+(\.\d+)?|\.\d+))?$").unwrap();
    raw.split(",")
        .map(
            |part| match part_pattern.captures(part.trim().to_lowercase().as_str()) {
                None => panic!("'{part}' is invalid!"),
                Some(captures) => (
                    find_product_name(products, &captures.get(1).unwrap().as_str().to_string()),
                    captures.get(3).map(|m| m.as_str().parse().unwrap()),
                ),
            },
        )
        .collect()
}

fn parse_product_index_list(products: &HashSet<String>, raw: &String) -> HashMap<String, usize> {
    let part_pattern = Regex::new(r"([^:]*):\s*(\d+)").unwrap();
    raw.split(",")
        .map(
            |part| match part_pattern.captures(part.trim().to_lowercase().as_str()) {
                None => panic!("'{part}' is invalid!"),
                Some(captures) => (
                    find_product_name(products, &captures.get(1).unwrap().as_str().to_string()),
                    captures
                        .get(2)
                        .map(|m| m.as_str().parse().unwrap())
                        .unwrap(),
                ),
            },
        )
        .collect()
}

fn load_recipes(file: &str) -> (IndexedMap<String, Recipe>, HashSet<String>) {
    let recipe_list = serde_json::from_str::<Vec<Recipe>>(
        fs::read_to_string(file)
            .expect(format!("{} not found!", file).as_str())
            .as_str(),
    )
    .expect(format!("{} is in an invalid format!", file).as_str());

    let recipe_map = recipe_list
        .clone()
        .into_iter()
        .map(|recipe| {
            recipe
                .products
                .iter()
                .map(|(product, _)| (product.clone(), recipe.clone()))
                .collect::<Vec<_>>()
        })
        .flatten()
        .into();

    let ingredient_set = recipe_list
        .into_iter()
        .map(|recipe| {
            recipe
                .ingredients
                .into_iter()
                .map(|(ingredient, _)| ingredient)
                .chain(
                    recipe
                        .products
                        .into_iter()
                        .map(|(product, _)| product)
                        .collect::<Vec<_>>(),
                )
        })
        .flatten()
        .collect();

    (recipe_map, ingredient_set)
}

/// Satisfactory Factory Planning Utility
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Product(s) to create, in the form `<name>[:rate][,<name>[:rate][...]]` etc.
    want: String,

    /// Ingredients that you have access to, in the form `<name>[:rate][,<name>[:rate][...]]` etc.
    have: Option<String>,

    /// Convert final machine counts to perfect split whole numbers, and list the underclocks for them
    #[arg(long, short = 'p', action = ArgAction::SetTrue)]
    show_perfect_splits: bool,

    /// If not enough input resources are available, then resupply more to fulfill the requested quota, instead of limiting the output totals
    #[arg(long, short = 's', action = ArgAction::SetTrue)]
    resupply_insufficient: bool,

    /// Specify a custom config file for crafting recipes
    #[arg(long, short = 'c', default_value = "recipes.json")]
    recipe_config: String,

    /// List all recipes that produce the given product
    #[arg(long, short = 'l', action = ArgAction::SetTrue)]
    list_recipes: bool,

    /// Provide overrides to existing recipes by passing a list of products and the associated recipe index to use to manufacture said product.
    /// Syntax is name:index[,name:index[,...]]
    #[arg(long, short = 'r')]
    recipes: Option<String>,

    /// !! EXPERIMENTAL !! Allow the reuse of byproduct outputs from the system as inputs
    #[arg(long, short = 'b', action = ArgAction::SetTrue)]
    reuse_byproducts: bool,
}

fn main() {
    // parse arguments
    #[cfg(not(debug_assertions))]
    let args = Args::parse();
    #[cfg(debug_assertions)]
    let args = Args::parse_from(vec!["_", "gas filter"]);

    // compute recipe map
    let (mut recipes, product_set) = load_recipes(&args.recipe_config);

    // parse lists of desired outputs
    let want_list = parse_product_list(&product_set, &args.want);

    if args.list_recipes {
        // list all recipes for the passed product
        for (product, _) in want_list {
            println!("{}:", product);
            match recipes.map.get(&product) {
                None => println!(" * No recipes for this product."),
                Some(recipe_set) => {
                    for (i, recipe) in recipe_set.iter().enumerate() {
                        println!(" {}.", i + 1);
                        println!("    Ingredients:");
                        for (ingredient, quantity) in recipe.ingredients.iter() {
                            println!("     - {:.2} {}/min", quantity, ingredient);
                        }
                        println!("    Products:");
                        for (product, quantity) in recipe.products.iter() {
                            println!("     - {:.2} {}/min", quantity, product);
                        }
                        println!();
                    }
                }
            }
        }
    } else {
        // parse list of available inputs
        let have_list = args.have.map_or_else(
            || Vec::new(),
            |have| parse_product_list(&product_set, &have),
        );

        // read recipe overrides
        args.recipes.map(|recipe_overrides| {
            recipes.index.extend(
                parse_product_index_list(&product_set, &recipe_overrides)
                    .into_iter()
                    .map(|(product, index)| (product, index - 1)),
            );
        });

        // Compute recipe dependencies
        let (tree, totals) = resolve_dependency_trees(
            &recipes,
            want_list,
            have_list,
            args.resupply_insufficient,
            args.reuse_byproducts,
        );

        // Display tree
        println!();
        println!("Tree:");
        for node in tree {
            println!("{node}");
        }

        // Display totals
        println!(
            "{}",
            DependencyResolutionTotalsDisplay {
                totals: &totals,
                show_perfect_splits: args.show_perfect_splits
            }
        );
    }
}
