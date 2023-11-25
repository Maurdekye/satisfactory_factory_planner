use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, fs};

/// Satisfactory Factory Planning Utility
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Product(s) to create, in the form `<name>[:rate][,<name>[:rate][...]]` etc.
    #[arg(required = true)]
    want: String,

    /// Ingredients that you have access to, in the form `<name>[:rate][,<name>[:rate][...]]` etc.
    have: Option<String>,

    /// Convert final machine counts to perfect split whole numbers, and list the underclocks for them
    #[arg(long, short, action = clap::ArgAction::SetTrue)]
    show_perfect_splits: bool,

    /// If not enough resources are available, then resupply more to fulfill the requested quota, instead of limiting the output totals
    #[arg(long, short, action = clap::ArgAction::SetTrue)]
    resupply_insufficient: bool,

    /// Config file containing crafting recipes
    #[arg(long, short, default_value = "recipes.json")]
    recipe_config: String
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
        ingredients: Vec<Product>,
        byproducts: Vec<(String, f32)>,
    },
    Supply,
    Byproduct,
}

#[derive(Clone, Debug)]
struct Product {
    name: String,
    unsupplied: f32,
    sources: Vec<(Source, f32)>,
}

impl Product {
    fn adjust_quantities(&mut self, adjustment: f32) {
        self.unsupplied *= adjustment;
        for (ref mut source, ref mut quantity) in self.sources.iter_mut() {
            *quantity *= adjustment;
            match source {
                Source::Recipe {
                    ingredients,
                    byproducts,
                    ..
                } => {
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
        for (source, quantity) in &self.product.sources {
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
                            "{:>indent$} > {:.2} {}",
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
                        "{:>indent$} < {:.2} {}",
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

trait DefaultDict<K: Eq + PartialEq + std::hash::Hash + Clone, V: Default> {
    fn get_default(&mut self, key: &K) -> &mut V;
}

impl<K: Eq + PartialEq + std::hash::Hash + Clone, V: Default> DefaultDict<K, V> for HashMap<K, V> {
    fn get_default(&mut self, key: &K) -> &mut V {
        self.entry(key.clone()).or_insert(V::default())
    }
}

#[derive(Debug)]
struct DependencyResolutionTotals {
    inputs: HashMap<String, f32>,
    intermediate_ingredients: HashMap<String, f32>,
    outputs: HashMap<String, f32>,
    byproducts: HashMap<String, f32>,
    machines: HashMap<String, HashMap<String, f32>>,
}

impl DependencyResolutionTotals {
    fn new() -> DependencyResolutionTotals {
        DependencyResolutionTotals {
            inputs: HashMap::new(),
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
            for (_, quantity) in &product.sources {
                *self.outputs.get_default(&product.name) += quantity;
            }

            // tally sub-nodes
            self.tally_node(product);
        })
    }

    fn tally_node(&mut self, node: &Product) {
        for (source, _) in &node.sources {
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
                        for (sub_source, quantity) in &product.sources {
                            match sub_source {
                                Source::Recipe { .. } => {
                                    *self.intermediate_ingredients.get_default(&product.name) +=
                                        quantity;
                                    self.tally_node(product);
                                }
                                Source::Supply => {
                                    *self.inputs.get_default(&product.name) += quantity;
                                }
                                Source::Byproduct => (),
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
        for (heading, product_list) in vec![
            ("Input ingredients:", &self.totals.inputs),
            (
                "Intermediate ingredients:",
                &self.totals.intermediate_ingredients,
            ),
            ("Output products:", &self.totals.outputs),
            ("Byproducts:", &self.totals.byproducts),
        ] {
            if !product_list.is_empty() {
                writeln!(f, "{heading}")?;
                for (product, quantity) in product_list.iter() {
                    writeln!(f, " * {:.2} {}", quantity, product)?;
                }

                writeln!(f)?;
            }
        }

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

/// this algorithm is an unholy abomination
fn nearest_perfect_split(base_machine_count: u32) -> Option<(u32, u32, u32)> {
    let uceil = |x: f32| x.ceil() as u32;
    let log_2: f32 = 2.0f32.ln();
    let log_3: f32 = 3.0f32.ln();
    let log2_log3: f32 = log_2 / log_3;

    let log_c: f32 = (base_machine_count as f32).ln();
    let b: f32 = log_c / log_3;
    let f = |x: u32| uceil((b - log2_log3 * x as f32).max(0.0));
    let mut result: Option<(u32, u32, u32)> = None;
    let mut closest_dist: Option<u32> = None;
    let mut x_pow2: u32 = 1;
    let mut last_y: u32 = f(0);
    let mut y_pow3: u32 = 3u32.pow(last_y);
    for x in 0..=uceil(log_c / log_2) {
        let y = f(x);
        if y != last_y {
            if y == last_y - 1 {
                y_pow3 /= 3;
            } else {
                y_pow3 = 3u32.pow(y);
            }
            last_y = y;
        }
        let split_value = x_pow2 * y_pow3;
        let new_dist = split_value - base_machine_count;
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
    recipes: &HashMap<String, Recipe>,
    product: &mut Product,
    available_ingredients: &Vec<String>,
) {
    match if available_ingredients.contains(&product.name) {
        None
    } else {
        recipes.get(&product.name)
    } {
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
                product.sources.push((
                    Source::Recipe {
                        machine: recipe.machine.clone(),
                        machine_quantity: production_ratio,

                        byproducts: recipe
                            .products
                            .iter()
                            .filter_map(|(recipe_product, quantity)| {
                                if *recipe_product != product.name {
                                    Some((recipe_product.clone(), quantity * production_ratio))
                                } else {
                                    None
                                }
                            })
                            .collect(),

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
                                );
                                inner_product
                            })
                            .collect(),
                    },
                    product.unsupplied,
                ));
            } else {
                for (source, _) in product.sources.iter_mut() {
                    match source {
                        Source::Recipe { ingredients, .. } => {
                            for ingredient in ingredients.iter_mut() {
                                resolve_product_dependencies(
                                    recipes,
                                    ingredient,
                                    available_ingredients,
                                )
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        None => {
            if product.unsupplied > 0.0 {
                product.sources.push((Source::Supply, product.unsupplied));
            }
        }
    };
    product.unsupplied = 0.0;
}

fn apply_insufficient_supply_proportions(
    recipes: &HashMap<String, Recipe>,
    product: &mut Product,
    resupply_proportions: &HashMap<String, f32>,
) {
    for (source, quantity) in product.sources.iter_mut() {
        match source {
            Source::Supply => {
                resupply_proportions.get(&product.name).map(|proportion| {
                    let new_quantity = *quantity * proportion;
                    product.unsupplied = *quantity - new_quantity;
                    *quantity = new_quantity;
                });
            }
            Source::Recipe { ingredients, .. } => {
                for ingredient in ingredients.iter_mut() {
                    apply_insufficient_supply_proportions(
                        recipes,
                        ingredient,
                        resupply_proportions,
                    );
                }
            }
            _ => (),
        }
    }
    product.sources.retain(|(_, quantity)| *quantity > 0.0);
}

fn compute_supply_proportions(
    totals: &DependencyResolutionTotals,
    ingredients: &HashMap<String, Option<f32>>,
) -> Vec<(String, f32)> {
    ingredients
        .iter()
        .filter_map(
            |(ingredient, maybe_quantity)| match totals.inputs.get(ingredient) {
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
    recipes: &HashMap<String, Recipe>,
    products: Vec<(String, Option<f32>)>,
    ingredients: Vec<(String, Option<f32>)>,
    resupply_insufficient: bool,
) -> (Vec<Product>, DependencyResolutionTotals) {
    let mut ingredients = ingredients.into_iter().collect::<HashMap<_, _>>();
    // dbg!(&ingredients);

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
        // dbg!(&trees);

        if !trees.is_empty() {
            let ingredient_set = ingredients.iter().map(|(i, _)| i.clone()).collect();
            // dbg!(&ingredient_set);
            for tree in &mut trees {
                resolve_product_dependencies(recipes, tree, &ingredient_set);
            }
            // dbg!(&trees);

            let mut totals = DependencyResolutionTotals::from(&trees);
            // dbg!(&totals);

            let initial_supply_proportions = compute_supply_proportions(&totals, &ingredients);
            // dbg!(&initial_supply_proportions);

            let insufficient_ingredients = initial_supply_proportions
                .clone()
                .into_iter()
                .filter(|(_, proportion)| *proportion < 1.0)
                .collect::<HashMap<_, _>>();
            // dbg!(&insufficient_ingredients);

            // adjust output if some provided quantities are insufficient
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
                    // dbg!(&ingredient_set_sans_resupplies);

                    for tree in &mut trees {
                        apply_insufficient_supply_proportions(
                            recipes,
                            tree,
                            &insufficient_ingredients,
                        );
                        resolve_product_dependencies(
                            recipes,
                            tree,
                            &ingredient_set_sans_resupplies,
                        );
                    }
                    // dbg!(&quantity_requested_trees);
                } else {
                    // adjust output to accommodate for lowest supplied ingredient
                    let lowest_supply = initial_supply_proportions
                        .iter()
                        .map(|(_, quantity)| *quantity)
                        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    // dbg!(&lowest_supply);

                    lowest_supply.map(|supply| {
                        for tree in &mut trees {
                            tree.adjust_quantities(supply);
                        }
                        // dbg!(&quantity_requested_trees);
                    });
                }
                totals = DependencyResolutionTotals::from(&trees);
                // dbg!(&totals);
            }

            // adjust available ingredients
            for (ingredient, used_quantity) in totals.inputs.iter() {
                if ingredients.contains_key(ingredient) {
                    ingredients.entry(ingredient.clone()).and_modify(|entry| {
                        *entry = entry
                            .map(|available_quantity| (available_quantity - used_quantity).max(0.0))
                    });
                }
            }
            // dbg!(&ingredients);
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
        // dbg!(&trees);

        if !trees.is_empty() {
            let ingredient_set = ingredients.iter().map(|(i, _)| i.clone()).collect();
            // dbg!(&ingredient_set);
            for tree in &mut trees {
                resolve_product_dependencies(recipes, tree, &ingredient_set);
            }
            // dbg!(&trees);

            let totals = DependencyResolutionTotals::from(&trees);
            // dbg!(&totals);

            let initial_supply_proportions = compute_supply_proportions(&totals, &ingredients);
            // dbg!(&initial_supply_proportions);

            // adjust output to acommodate for lowest supplied ingredient
            let lowest_supply = initial_supply_proportions
                .iter()
                .map(|(_, quantity)| *quantity)
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            // dbg!(&lowest_supply);

            lowest_supply.map(|supply| {
                for tree in &mut trees {
                    tree.adjust_quantities(supply);
                }
                // dbg!(&trees);
            });
        }
        trees
    };

    let trees = quantity_requested_trees
        .into_iter()
        .chain(quantity_unrequested_trees.into_iter())
        .collect();
    // dbg!(&trees);

    let totals = DependencyResolutionTotals::from(&trees);
    // dbg!(&totals);

    (trees, totals)
}

fn parse_product_list(
    recipes: &HashMap<String, Recipe>,
    raw: &String,
) -> Vec<(String, Option<f32>)> {
    let part_pattern = Regex::new(r"^([^:]*)(:\s*(\d+(\.\d+)?|\.\d+))?$").unwrap();
    raw.split(",")
        .map(
            |part| match part_pattern.captures(part.trim().to_lowercase().as_str()) {
                None => panic!("'{part}' is invalid!"),
                Some(captures) => {
                    let raw_name = captures.get(1).unwrap().as_str().to_string();
                    (
                        recipes
                            .iter()
                            .map(|(full_name, _)| full_name)
                            .find(|full_name| full_name.to_lowercase() == raw_name)
                            .unwrap_or(&raw_name)
                            .clone(),
                        captures.get(3).map(|m| m.as_str().parse().unwrap()),
                    )
                }
            },
        )
        .collect()
}

fn load_recipes(file: &str) -> HashMap<String, Recipe> {
    serde_json::from_str::<Vec<Recipe>>(fs::read_to_string(file).unwrap().as_str())
        .unwrap()
        .into_iter()
        .map(|recipe| {
            recipe
                .products
                .iter()
                .map(|(product, _)| (product.clone(), recipe.clone()))
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect()
}

fn main() {

    // parse arguments
    let args = Args::parse();
    // let args = Args::parse_from(vec!["_", "computer:2.5,cable", "copper ingot:130"]);

    // compute recipe map
    let recipes = load_recipes(&args.recipe_config);

    // parse lists of desired outputs and available inputs
    let want_list = parse_product_list(&recipes, &args.want);
    let have_list = args
        .have
        .map_or_else(|| Vec::new(), |have| parse_product_list(&recipes, &have));

    // Compute recipe dependencies
    let (tree, totals) =
        resolve_dependency_trees(&recipes, want_list, have_list, args.resupply_insufficient);

    // Display tree
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
