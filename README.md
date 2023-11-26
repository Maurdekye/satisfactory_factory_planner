# Satisfactory Factory Planning Utility

This is a command line utility written in Rust that helps you plan factories in Satisfactory. 

Tell it what you want to create and at what rates, and it will tell you what machines and input resources you need in order to create those products at those rates.

Additionally, tell it what resources you have access to, and it will adjust its output to compensate. If you provide information on what rate you produce those resources, it can tell you how much you can make with those resources, and which resource is the bottleneck to your production.

## Sample Output

```
>satisfactory_factory_planner.exe "computer: 5"

Tree:
 * 5.00 Computer: 2.00 Manufacturer
   * 50.00 Circuit Board: 6.67 Assembler
     * 100.00 Copper Sheet: 10.00 Constructor
       * 200.00 Copper Ingot: 6.67 Smeltery
         - 200.00 Copper Ore
     * 200.00 Plastic: 10.00 Refinery
       - 300.00 Crude Oil
   * 45.00 Cable: 1.50 Constructor
     * 90.00 Wire: 3.00 Constructor
       * 45.00 Copper Ingot: 1.50 Smeltery
         - 45.00 Copper Ore
   * 90.00 Plastic: 4.50 Refinery
     - 135.00 Crude Oil
   * 260.00 Screw: 6.50 Constructor
     * 65.00 Iron Rod: 4.33 Constructor
       * 65.00 Iron Ingot: 2.17 Smeltery
         - 65.00 Iron Ore

Input ingredient totals:
 * 65.00 Iron Ore
 * 245.00 Copper Ore
 * 435.00 Crude Oil

Intermediate ingredient totals:
 * 65.00 Iron Rod
 * 260.00 Screw
 * 65.00 Iron Ingot
 * 45.00 Cable
 * 245.00 Copper Ingot
 * 290.00 Plastic
 * 50.00 Circuit Board
 * 100.00 Copper Sheet
 * 90.00 Wire

Output product totals:
 * 5.00 Computer

Byproducts:
 * 20.00 Heavy Oil Residue

Machines:
 * Assembler
   - 6.67 for Circuit Boards
 * Manufacturer
   - 2.00 for Computers
 * Refinery
   - 14.50 for Plastics
 * Constructor
   - 3.00 for Wires
   - 10.00 for Copper Sheets
   - 6.50 for Screws
   - 1.50 for Cables
   - 4.33 for Iron Rods
 * Smeltery
   - 2.17 for Iron Ingots
   - 8.17 for Copper Ingots
```

## Example Usages

* `satisfactory_factory_planner.exe "iron ingot: 120"` - Figure out how much iron ore & how many smelters you need to produce 120 iron ingots per minute

* `[.exe] cable` - Plan a basic factory that produces 30 cables / minute (if no output rate and no input ingredient rates are provided, the rate produced by 1 machine is assumed)

* `[.exe] "modular frame: 10" "copper ingot, reinforced iron plate"` - Plan a factory that produces 10 modular frames / minute, taking into account that you already have infrastructure to produce copper ingots and reinforced iron plates

* `[.exe] "encased industrial beam" "steel ingot: 60, concrete: 90"` - Figure out how many encased industrial beams you can produce, if you are producing 60 steel ingots and 90 concrete per minute

* `[.exe] "motor: 5" "iron ingot: 60"` - Determine if you are able to produce 5 motors / minute, given you have access to only 60 iron ingots / minute. If not, the reported output quantity will be lower. (in this case, you will see a result for a factory that only produces ~2.67 motors / minute)

* `[.exe] "motor: 5, heavy modular frame: 10, cable: 50, plastic: 50"` - See what kind of factory would be needed to produce the ingredients for a single manufaturer every minute

## Installation

[Download the latest release zip](https://github.com/Maurdekye/satisfactory_factory_planner/releases), unzip `satisfactory_factory_planner.exe` and `recipes.json` to a folder, and run the executable from a command line.

## Options

- `-s, --show-perfect-splits` - Convert final machine counts to perfect split whole numbers, and list the underclocks for them

#### Example:

```
>[.exe] "steel beam: 50" --show-perfect-splits

Tree:
 * 50.00 Steel Beam: 3.33 Constructor
   * 200.00 Steel Ingot: 4.44 Foundry
     - 200.00 Iron Ore
     - 200.00 Coal Ore

Input Ingredients:
 * 200.00 Coal Ore
 * 200.00 Iron Ore

Intermediate Ingredients:
 * 200.00 Steel Ingot

Output Products:
 * 50.00 Steel Beam

Machines:
 * Foundry
   - 4.44 for Steel Ingots, or 2^1 * 3^1 = 6 at 74.07%
 * Constructor
   - 3.33 for Steel Beams, or 2^2 * 3^0 = 4 at 83.33%
```

- `-r, --resupply-insufficient` - If not enough input resources are available, then resupply more to fulfill the requested quota, instead of limiting the output totals

#### Example:

```
>[.exe] "steel beam: 60" "steel ingot: 160" --resupply-insufficient

Tree:
 * 60.00 Steel Beam: 4.00 Constructor
   - 160.00 Steel Ingot
   * 80.00 Steel Ingot: 1.78 Foundry
     - 80.00 Iron Ore
     - 80.00 Coal Ore

Input Ingredients:
 * 160.00 Steel Ingot
 * 80.00 Iron Ore
 * 80.00 Coal Ore

Intermediate Ingredients:
 * 80.00 Steel Ingot

Output Products:
 * 60.00 Steel Beam

Machines:
 * Constructor
   - 4.00 for Steel Beams
 * Foundry
   - 1.78 for Steel Ingots
```

- `-c, --recipe-config <RECIPE_CONFIG>`  
Specify a custom config file for crafting recipes [default: recipes.json]

#### Example:

```
>[.exe] "steel beam: 60" --recipe-config my-recipes.json

Tree:
 * 60.00 Steel Beam: 4.00 Constructor
   * 240.00 Steel Ingot: 4.00 Foundry
     * 160.00 Iron Ingot: 5.33 Smeltery
       - 160.00 Iron Ore
     - 160.00 Coal Ore

Input Ingredients:
 * 160.00 Coal Ore
 * 160.00 Iron Ore

Intermediate Ingredients:
 * 160.00 Iron Ingot
 * 240.00 Steel Ingot

Output Products:
 * 60.00 Steel Beam

Machines:
 * Foundry
   - 4.00 for Steel Ingots
 * Constructor
   - 4.00 for Steel Beams
 * Smeltery
   - 5.33 for Iron Ingots
```
- `-b, --reuse-byproducts` - !! EXPERIMENTAL !! Allow the reuse of byproduct outputs from the system as inputs

#### Example:
```
>[.exe] "plastic: 45, fuel: 20" --reuse-byproducts

Tree:
 * 45.00 Plastic: 2.25 Refinery
   - 67.50 Crude Oil
 > 22.50 Heavy Oil Residue

 * 20.00 Fuel: 0.50 Refinery
   < 22.50 Heavy Oil Residue
   * 7.50 Heavy Oil Residue: 0.38 Refinery
     - 11.25 Crude Oil
   > 7.50 Rubber

Input Ingredients:
 * 78.75 Crude Oil

Intermediate Ingredients:
 * 7.50 Heavy Oil Residue

Output Products:
 * 20.00 Fuel
 * 45.00 Plastic

Byproducts:
 * 7.50 Rubber

Machines:
 * Refinery
   - 2.25 for Plastics
   - 0.38 for Heavy Oil Residues
   - 0.50 for Fuels
```

## Known Flaws

* ~~Byproducts are not utilized in the production chain~~ **Enable experimental byproduct reuse with the `--reuse-byproducts` flag**
* If multiple recipes to acquire a given resource exist, the program will choose one arbitrarily and use it exclusively. To ensure the program uses a specific recipe, you'll have to edit `recipes.json` and remove all alternative recipes.

## Other Notes

The recipe information the program draws from is contained inside `recipes.json`. Currently, the file only contains recipes up to Tier 6, as that's how far my friend and I are into our current playthrough. Additional recipes, if you like, can be added by modifying `recipes.json`. This is left as an exercise to the user :). Furthermore, no alternative unlockable recipes are programmed into the file, on account of the aformentioned flaw with regards to alternative recipes. If you would like the program to use an alternative recipe, I would recommend removing the basic recipe from the file, and replacing it with the alternative one.

---
\
I hope you enjoy using my program!~ ❤️
