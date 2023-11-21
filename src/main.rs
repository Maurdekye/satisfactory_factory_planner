use clap::Parser;
use serde::Deserialize;

use std::{collections::HashMap, fmt::Display, fs};

#[derive(Parser, Debug)]
struct Args {
    /// Product to create
    #[arg(required = true)]
    product: String,

    /// Amount to create
    quantity: Option<f32>

    // /// Quantities of input resources available in the form: <name>:<quantity>;<name>:<quantity> etc.
    // #[arg(short, long)]
    // inputs: Option<String>,
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
}

#[derive(Clone)]
struct Product {
    name: String,
    quantity: f32,
    source: Option<Source>,
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

struct DependencyResolutionResult {
    dependency_tree: Product,
    product_totals: HashMap<String, f32>,
    machine_totals: HashMap<String, HashMap<String, f32>>,
    byproducts: HashMap<String, f32>,
}

impl Display for DependencyResolutionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Dependency tree:")?;
        self.dependency_tree.fmt(f)?;
        writeln!(f)?;

        writeln!(f, "Ingredient totals:")?;
        for (product, quantity) in self.product_totals.iter() {
            writeln!(f, " * {:.2} {}s", quantity, product)?;
        }

        writeln!(f, "Machines:")?;
        for (machine, machine_products) in self.machine_totals.iter() {
            writeln!(f, " * {}s", machine)?;
            for (product, quantity) in machine_products.iter() {
                writeln!(f, "   - {:.2} for {}s", quantity, product)?;
            }
        }

        if !self.byproducts.is_empty() {
            writeln!(f, "Byproducts:")?;
            for (product, quantity) in self.byproducts.iter() {
                writeln!(f, " * {:.2} {}s", quantity, product)?;
            }
        }

        Ok(())
    }
}

fn resolve_product_dependencies_(
    recipe_map: &HashMap<String, Recipe>,
    product: Product,
    dependency_resolution_result: &mut DependencyResolutionResult,
) -> Product {
    let mut product = product;
    match recipe_map.get(&product.name) {
        Some(recipe) => {
            // determine production ratio / log byproducts
            let mut maybe_production_ratio = None;
            for (recipe_product, quantity) in recipe.products.iter() {
                if *recipe_product == product.name {
                    maybe_production_ratio = Some(product.quantity / quantity);
                } else {
                    *dependency_resolution_result
                        .byproducts
                        .entry(recipe_product.clone())
                        .or_insert(0.0) += quantity;
                }
            }

            let production_ratio =
                maybe_production_ratio.expect("Recipe in value missing product from its key?!");

            // log machine requirement
            *dependency_resolution_result
                .machine_totals
                .entry(recipe.machine.clone())
                .or_insert(HashMap::new())
                .entry(product.name.clone())
                .or_insert(0.0) += production_ratio;

            // compute ingredient dependencies
            product.source = Some(Source {
                machine: recipe.machine.clone(),
                machine_quantity: production_ratio,
                ingredients: recipe
                    .ingredients
                    .iter()
                    .map(|(recipe_product, quantity)| {
                        resolve_product_dependencies_(
                            recipe_map,
                            Product {
                                name: recipe_product.clone(),
                                quantity: quantity * production_ratio,
                                source: None,
                            },
                            dependency_resolution_result,
                        )
                    })
                    .collect(),
            });
        }
        None => {
            // log input product total
            *dependency_resolution_result
                .product_totals
                .entry(product.name.clone())
                .or_insert(0.0) += product.quantity;
        }
    };
    product
}

fn resolve_product_dependencies(
    recipe_map: &HashMap<String, Recipe>,
    product: String,
    quantity: f32,
) -> DependencyResolutionResult {
    let product = Product {
        name: product,
        quantity: quantity,
        source: None,
    };
    let mut dependency_resolution_result = DependencyResolutionResult {
        dependency_tree: product.clone(),
        product_totals: HashMap::new(),
        machine_totals: HashMap::new(),
        byproducts: HashMap::new(),
    };
    dependency_resolution_result.dependency_tree =
        resolve_product_dependencies_(recipe_map, product, &mut dependency_resolution_result);
    dependency_resolution_result
}

fn main() {
    // parse arguments
    // let args = Args::parse();
    let args = Args::parse_from(vec!["satisfactory_factory_planner", "Computer", "2.5"]);

    // red in recipe file
    let recipes: Vec<Recipe> =
        serde_json::from_str(fs::read_to_string("recipes.json").unwrap().as_str()).unwrap();

    // Compute recipe map
    let recipe_map: HashMap<String, Recipe> = recipes
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

    // Compute recipe dependencies
    let result = resolve_product_dependencies(&recipe_map, args.product, args.quantity.unwrap());

    // Display result
    println!("{result}");
}
