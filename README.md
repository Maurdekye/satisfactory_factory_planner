# Satisfactory Factory Planning Utility

This is a command line utility written in Rust that helps you plan factories in Satisfactory. 

Tell it what you want to create and at what rates, and it will tell you what machines and input resources you need in order to create those products at those rates.

Additionally, tell it what resources you have access to, and it will adjust its output to compensate. If you provide information on what rate you produce those resources, it can tell you how much you can make with those resources, and which resource is the bottleneck to your production.

--- 

### Example Usages

* `satisfactory_factory_planner.exe "iron ingot: 120"` - Figure out how much iron ore & how many smelters you need to produce 120 iron ingots per minute

* `[.exe] cable` - Plan a basic factory that produces 30 cables / minute (if no output rate and no input ingredient rates are provided, the rate produced by 1 machine is assumed)

* `[.exe] "modular frame: 10" "copper ingot, reinforced iron plate"` - Plan a factory that produces 10 modular frames / minute, taking into account that you already have infrastructure to produce copper ingots and reinforced iron plates

* `[.exe] "encased industrial beam" "steel ingot: 60, concrete: 90"` - Figure out how many encased industrial beams you can produce, if you are producing 60 steel ingots and 90 concrete per minute

* `[.exe] "motor: 5" "iron ingot: 60"` - Determine if you are able to produce 5 motors / minute, given you have access to only 60 iron ingots / minute. If not, the reported output quantity will be lower. (in this case, you will see a result for a factory that only produces ~2.67 motors / minute)

* `[.exe] "motor: 5, heavy modular frame: 10, cable: 50, plastic: 50"` - See what kind of factory would be needed to produce the ingredients for a single manufaturer every minute

---

### Installation

[Download the latest release zip](https://github.com/Maurdekye/satisfactory_factory_planner/releases), unzip `satisfactory_factory_planner.exe` and `recipes.json` to a folder, and run the executable from a command line.

---

### Known Flaws

* Byproducts are not utilized in the production chain; they are simply ignored
* If multiple recipes to acquire a given resource exist, the program will choose one arbitrarily and use it exclusively. To ensure the program uses a specific recipe, you'll have to edit `recipes.json` and remove all alternative recipes.

---

### Other Notes

The recipe information the program draws from is contained inside `recipes.json`. Currently, the file only contains recipes up to Tier 6, as that's how far my friend and I are into our current playthrough. Additional recipes, if you like, can be added by modifying `recipes.json`. This is left as an exercise to the user :). Furthermore, no alternative unlockable recipes are programmed into the file, on account of the aformentioned flaw with regards to alternative recipes. If you would like the program to use an alternative recipe, I would recommend removing the basic recipe from the file, and replacing it with the alternative one.

---

I hope you enjoy using my program!~ ❤️