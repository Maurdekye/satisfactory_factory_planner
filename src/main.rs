use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, fs};

#[derive(Parser, Debug)]
struct Args {
    /// Product(s) to create, in the form `<name>[:rate][,<name>[:rate]]` etc.
    #[arg(required = true)]
    want: String,

    /// Ingredients that you have access to, in the form `<name>[:rate][,<name>[:rate]]` etc.
    have: Option<String>,
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

impl Product {
    fn recursive_adjust_totals(&mut self, adjust_by: f32) {
        self.quantity *= adjust_by;
        self.source
            .iter_mut()
            .map(|source| {
                source.machine_quantity *= adjust_by;
                source
                    .ingredients
                    .iter_mut()
                    .for_each(|inner_product| inner_product.recursive_adjust_totals(adjust_by));
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
    dependency_trees: Vec<Product>,
    ingredient_totals: HashMap<String, f32>,
    machine_totals: HashMap<String, HashMap<String, f32>>,
    byproducts: HashMap<String, f32>,
}

impl DependencyResolutionResult {
    fn adjust_values(&mut self, adjustment: f32) {
        self.dependency_trees
            .iter_mut()
            .for_each(|product| product.recursive_adjust_totals(adjustment));
        self.ingredient_totals = self
            .ingredient_totals
            .iter()
            .map(|(ingredient, quantity)| (ingredient.clone(), quantity * adjustment))
            .collect();
        self.machine_totals = self
            .machine_totals
            .iter()
            .map(|(machine, ingredient_totals)| {
                (
                    machine.clone(),
                    ingredient_totals
                        .into_iter()
                        .map(|(ingredient, quantity)| (ingredient.clone(), quantity * adjustment))
                        .collect(),
                )
            })
            .collect();
        self.byproducts = self
            .byproducts
            .iter()
            .map(|(ingredient, quantity)| (ingredient.clone(), quantity * adjustment))
            .collect();
    }
}

impl Display for DependencyResolutionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Tree:")?;
        for dependency_tree in self.dependency_trees.iter() {
            dependency_tree.fmt(f)?;
        }

        writeln!(f)?;

        writeln!(f, "Ingredient totals:")?;
        for (product, quantity) in self.ingredient_totals.iter() {
            writeln!(f, " * {:.2} {}", quantity, product)?;
        }

        writeln!(f)?;

        writeln!(f, "Machines:")?;
        for (machine, machine_products) in self.machine_totals.iter() {
            writeln!(f, " * {}", machine)?;
            for (product, quantity) in machine_products.iter() {
                writeln!(f, "   - {:.2} for {}s", quantity, product)?;
            }
        }

        writeln!(f)?;

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
    recipes: &HashMap<String, Recipe>,
    product: Product,
    ingredients: &Vec<String>,
    dependency_resolution_result: &mut DependencyResolutionResult,
) -> Product {
    let mut product = product;
    match if ingredients.contains(&product.name) {
        None
    } else {
        recipes.get(&product.name)
    } {
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
                            recipes,
                            Product {
                                name: recipe_product.clone(),
                                quantity: quantity * production_ratio,
                                source: None,
                            },
                            ingredients,
                            dependency_resolution_result,
                        )
                    })
                    .collect(),
            });
        }
        None => {
            // log input product total
            *dependency_resolution_result
                .ingredient_totals
                .entry(product.name.clone())
                .or_insert(0.0) += product.quantity;
        }
    };
    product
}

fn resolve_product_dependencies(
    recipes: &HashMap<String, Recipe>,
    products: Vec<(String, Option<f32>)>,
    ingredients: Vec<(String, Option<f32>)>,
) -> DependencyResolutionResult {
    let mut dependency_resolution_result = DependencyResolutionResult {
        dependency_trees: vec![],
        ingredient_totals: HashMap::new(),
        machine_totals: HashMap::new(),
        byproducts: HashMap::new(),
    };
    let ingredient_names = ingredients
        .iter()
        .map(|(product, _)| product.clone())
        .collect();

    // construct dependency tree & tally ingredient, machine, + byproduct quantities
    dependency_resolution_result.dependency_trees = products
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
                &mut dependency_resolution_result,
            )
        })
        .collect();

    // compute greatest excess of individual required ingredients vs available ingredients
    let greatest_excess = ingredients
        .iter()
        .filter_map(|(ingredient, maybe_quantity)| {
            match dependency_resolution_result
                .ingredient_totals
                .get(ingredient)
            {
                Some(ingredient_quantity) => maybe_quantity.map(|available_ingredient_quantity| {
                    (
                        ingredient.clone(),
                        ingredient_quantity / available_ingredient_quantity,
                    )
                }),
                None => None,
            }
        })
        .map(|(_, quantity)| quantity)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // adjust totals based on excess & passed arguments
    greatest_excess.map(|excess| {
        if !products.iter().any(|(_, quantity)| quantity.is_some()) || excess > 1.0 {
            dependency_resolution_result.adjust_values(1.0 / excess);
        }
    });

    dependency_resolution_result
}

fn parse_product_list(
    recipes: &HashMap<String, Recipe>,
    raw: &String,
) -> Vec<(String, Option<f32>)> {
    let part_pattern = Regex::new(r"^([^:]*)(:(\d+(\.\d+)?|\.\d+))?$").unwrap();
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
                            .unwrap_or(&raw_name).clone(),
                        captures.get(3).map(|m| m.as_str().parse().unwrap()),
                    )
                }
            },
        )
        .collect()
}

fn main() {
    // parse arguments
    let args = Args::parse();

    // Compute recipe map
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

    let want_list = parse_product_list(&recipes, &args.want);
    let have_list = args.have.map(|s| parse_product_list(&recipes, &s));

    // Compute recipe dependencies
    let result = resolve_product_dependencies(
        &recipes,
        want_list,
        have_list.unwrap_or_else(|| Vec::new()),
    );

    // Display result
    println!("{result}");
}
