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
}

#[derive(Deserialize, Clone)]
struct Recipe {
    machine: String,
    ingredients: Vec<(String, f32)>,
    products: Vec<(String, f32)>,
}

#[derive(Clone)]
struct Source {
    machine: String,
    machine_quantity: f32,
    ingredients: Vec<Product>,
    byproducts: Vec<Product>,
}

#[derive(Clone)]
struct Product {
    name: String,
    quantity: f32,
    source: Option<Source>,
}

impl Product {
    fn adjust_quantities(&mut self, adjustment: f32) {
        self.quantity *= adjustment;
        self.source
            .iter_mut()
            .map(|source| {
                source.machine_quantity *= adjustment;
                source
                    .ingredients
                    .iter_mut()
                    .for_each(|inner_product| inner_product.adjust_quantities(adjustment));
                source
                    .byproducts
                    .iter_mut()
                    .for_each(|inner_product| inner_product.adjust_quantities(adjustment));
            })
            .count();
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
        match &self.product.source {
            Some(source) => {
                writeln!(
                    f,
                    "{:>indent$} * {:.2} {}: {:.2} {}",
                    "",
                    self.product.quantity,
                    self.product.name,
                    source.machine_quantity,
                    source.machine,
                    indent = self.indent,
                )?;
                for sub_product in source.ingredients.iter() {
                    ProductDisplay {
                        product: sub_product.clone(),
                        indent: self.indent + 2,
                    }
                    .fmt(f)?;
                }
                for byproduct in source.byproducts.iter() {
                    writeln!(
                        f,
                        "{:>indent$} > {:.2} {}",
                        "",
                        byproduct.quantity,
                        byproduct.name,
                        indent = self.indent,
                    )?;
                }
            }
            None => {
                writeln!(
                    f,
                    "{:indent$} - {:.2} {}",
                    "",
                    self.product.quantity,
                    self.product.name,
                    indent = self.indent
                )?;
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
            *self.outputs.get_default(&product.name) += product.quantity;

            // tally sub-nodes
            self.tally_node(product);
        })
    }

    fn tally_node(&mut self, node: &Product) {
        node.source.as_ref().map(|source| {
            // tally machine counts
            *self
                .machines
                .get_default(&source.machine)
                .get_default(&node.name) += source.machine_quantity;

            // tally byproducts
            source.byproducts.iter().for_each(|product| {
                *self.byproducts.get_default(&product.name) += product.quantity
            });

            // tally intermediate ingredients, inputs, and sub-nodes
            source.ingredients.iter().for_each(|product| {
                if product.source.is_some() {
                    *self.intermediate_ingredients.get_default(&product.name) += product.quantity;
                    self.tally_node(product);
                } else {
                    *self.inputs.get_default(&product.name) += product.quantity;
                }
            });
        });
    }
}

struct DependencyResolutionTotalsDisplay {
    totals: DependencyResolutionTotals,
    show_perfect_splits: bool,
}

impl Display for DependencyResolutionTotalsDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        for (heading, product_list) in vec![
            ("Input ingredients:", &self.totals.inputs),
            ("Intermediate ingredients:", &self.totals.intermediate_ingredients),
            ("Output products:", &self.totals.outputs),
            ("Byproducts:", &self.totals.byproducts)
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
                    let round_up = quantity.ceil() as usize;
                    let (splitters_2, splitters_3) = nearest_perfect_split(round_up).unwrap();
                    let perfect_split_quantity = 2usize.pow(splitters_2) * 3usize.pow(splitters_3);
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
fn nearest_perfect_split(c: usize) -> Option<(u32, u32)> {
    let log_2: f32 = 2.0f32.ln();
    let log_3: f32 = 3.0f32.ln();
    let log2_log3: f32 = log_2 / log_3;

    let log_c: f32 = (c as f32).ln();
    let b: f32 = log_c / log_3;
    let d: f32 = log_c / log_2;
    let f = |x: f32| b - log2_log3 * x;
    let dist = |x: f32, y: f32| ((-log2_log3) * x - y + b).abs();
    let mut closest = None;
    let mut closest_dist = None;
    for x in 0..=(d.ceil() as u32) {
        let y = f(x as f32).ceil() as i32;
        let new_dist = dist(x as f32, y as f32);
        if closest_dist.map_or(true, |c_dist| new_dist < c_dist) {
            closest = Some((x, y as u32));
            if new_dist == 0.0 {
                break;
            }
            closest_dist = Some(new_dist);
        }
    }
    closest
}

fn resolve_product_dependencies_(
    recipes: &HashMap<String, Recipe>,
    product: Product,
    ingredients: &Vec<String>,
) -> Product {
    let mut product = product;

    match if ingredients.contains(&product.name) {
        None
    } else {
        recipes.get(&product.name)
    } {
        Some(recipe) => {
            // determine production ratio
            let production_ratio = product.quantity
                / recipe
                    .products
                    .iter()
                    .find(|(recipe_product, _)| *recipe_product == product.name)
                    .expect("Recipe in value missing product from its key?!")
                    .1;

            // compute ingredient dependencies
            product.source = Some(Source {
                machine: recipe.machine.clone(),
                machine_quantity: production_ratio,

                byproducts: recipe
                    .products
                    .iter()
                    .filter_map(|(recipe_product, quantity)| {
                        if *recipe_product != product.name {
                            Some(Product {
                                name: recipe_product.clone(),
                                quantity: quantity * production_ratio,
                                source: None,
                            })
                        } else {
                            None
                        }
                    })
                    .collect(),

                ingredients: recipe
                    .ingredients
                    .iter()
                    .map(|(recipe_product, quantity)| {
                        resolve_product_dependencies_(
                            recipes,
                            Product {
                                name: recipe_product.clone(),
                                quantity: quantity * production_ratio,
                                source: None,
                            },
                            ingredients,
                        )
                    })
                    .collect(),
            });
        }
        None => (),
    };
    product
}

fn resolve_product_dependencies(
    recipes: &HashMap<String, Recipe>,
    products: Vec<(String, Option<f32>)>,
    ingredients: Vec<(String, Option<f32>)>,
) -> (Vec<Product>, DependencyResolutionTotals) {
    let ingredient_names = ingredients
        .iter()
        .map(|(product, _)| product.clone())
        .collect();

    // construct dependency tree
    let mut dependency_trees = products
        .iter()
        .map(|(product, quantity)| {
            resolve_product_dependencies_(
                recipes,
                Product {
                    quantity: quantity.unwrap_or_else(|| {
                        recipes.get(product).map_or(1.0, |recipe| {
                            recipe
                                .products
                                .iter()
                                .find(|(p, _)| p == product)
                                .map(|(_, q)| q.clone())
                                .unwrap_or(1.0)
                        })
                    }),
                    name: product.clone(),
                    source: None,
                },
                &ingredient_names,
            )
        })
        .collect();

    // tally ingredient, machine, + byproduct quantities
    let mut totals = DependencyResolutionTotals::from(&dependency_trees);

    // compute greatest excess of individual required ingredients vs available ingredients
    let greatest_excess = ingredients
        .iter()
        .filter_map(
            |(ingredient, maybe_quantity)| match totals.inputs.get(ingredient) {
                Some(ingredient_quantity) => maybe_quantity.map(|available_ingredient_quantity| {
                    (
                        ingredient.clone(),
                        ingredient_quantity / available_ingredient_quantity,
                    )
                }),
                None => None,
            },
        )
        .map(|(_, quantity)| quantity)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // adjust quantities based on excess & passed arguments
    greatest_excess.map(|excess| {
        if !products.iter().any(|(_, quantity)| quantity.is_some()) || excess > 1.0 {
            dependency_trees
                .iter_mut()
                .for_each(|tree| tree.adjust_quantities(1.0 / excess));
            totals = DependencyResolutionTotals::from(&dependency_trees);
        }
    });

    (dependency_trees, totals)
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

fn main() {
    // compute recipe map
    let recipes: HashMap<String, Recipe> =
        serde_json::from_str::<Vec<Recipe>>(fs::read_to_string("recipes.json").unwrap().as_str())
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
            .collect();

    // parse arguments
    let args = Args::parse();
    // let args = Args::parse_from(vec!["_", "plastic:300"]);

    let want_list = parse_product_list(&recipes, &args.want);
    let have_list = args
        .have
        .map_or_else(|| Vec::new(), |have| parse_product_list(&recipes, &have));

    // Compute recipe dependencies
    let (tree, totals) = resolve_product_dependencies(&recipes, want_list, have_list);

    // Display tree
    println!("Tree:");
    for node in tree {
        println!("{node}");
    }

    // Display totals
    println!(
        "{}",
        DependencyResolutionTotalsDisplay {
            totals: totals,
            show_perfect_splits: args.show_perfect_splits
        }
    );
}
