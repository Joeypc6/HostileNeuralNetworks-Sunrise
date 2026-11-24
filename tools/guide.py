"""The W guide's pages for Hostile Neural Networks (assets/hostilenetworks/ae2guide/, read by GuideME, ../guide-fork):
English pages and their Spanish translations (`_es_es/`), each short, with scenes of the machines and a whole Data
Center. Called by make_fork_content.py."""

import json

ID = "hostilenetworks"

PAGES = {
    "index.md": ("Hostile Neural Networks", 0, f"{ID}:blank_data_model", [f"{ID}:blank_data_model", f"{ID}:prediction_matrix"], """
# Hostile Neural Networks

Turn mobs into data, and data into their drops, without fighting them.

1. Right-click a mob with a **Model Framework** to attune a [Data Model](data_models.md) to it.
2. Put the model in a [Deep Learner](deep_learner.md) and fight that mob: each kill trains the model.
3. A [Simulation Chamber](sim_chamber.md) runs the model on Prediction Matrices and power.
4. A [Loot Fabricator](loot_fabricator.md) turns its Predictions into the drops you choose.
5. Later, a [Data Center](data_center.md) runs twenty-five Self Aware models at once.

<RecipeFor id="blank_data_model" />

<RecipeFor id="prediction_matrix" />
"""),
    "data_models.md": ("Data Models", 10, f"{ID}:data_model", [f"{ID}:data_model"], """
# Data Models

A Data Model holds what is known about one mob (and its variants). It has a **tier**: Faulty, Basic, Advanced,
Superior and Self Aware. Each tier needs more data and gives a better **accuracy**, the chance that a simulation
makes the mob's own Prediction. A Self Aware model always does.

Data comes from kills while the model is in a Deep Learner you carry, or from a Simulation Chamber in Training mode.
A Faulty model can't be simulated: train it to Basic first.

Hover a model for its tier, its data and its simulation cost.
"""),
    "deep_learner.md": ("Deep Learner", 20, f"{ID}:deep_learner", [f"{ID}:deep_learner"], """
# Deep Learner

Holds up to four Data Models. Right-click with it to open it; click a model in your inventory to put it in, and a
model in the learner to take it out. The arrows show each model: its mob, its tier, its accuracy and how many kills
the next tier needs.

Carry it (anywhere in your inventory) and every kill of a model's mob trains it. Held, it shows its models' progress
in the corner of the screen.

<RecipeFor id="deep_learner" />
"""),
    "sim_chamber.md": ("Simulation Chamber", 30, f"{ID}:sim_chamber", [f"{ID}:sim_chamber", f"{ID}:prediction",
                                                                       f"{ID}:overworld_prediction", f"{ID}:nether_prediction",
                                                                       f"{ID}:end_prediction", f"{ID}:twilight_prediction"], """
# Simulation Chamber

Put a Data Model in the slot on its left and Prediction Matrices in its input. With power (FE), each run takes one
matrix and makes the world's **Generalized Prediction** and, as the model's accuracy allows, the mob's own
**Prediction**.

- **Inference** mode (the eye) makes outputs. **Training** mode (the bottle) adds data to the model instead, and
  costs a fifth more power.
- The redstone button sets whether a signal turns it on or off.
- Hoppers and pipes put models and matrices in, and take the outputs out.

<GameScene zoom="4" interactive={true}>
  <ImportStructure src="scenes/machines.snbt" />
</GameScene>

<RecipeFor id="sim_chamber" />
"""),
    "loot_fabricator.md": ("Loot Fabricator", 40, f"{ID}:loot_fabricator", [f"{ID}:loot_fabricator", f"{ID}:fab_directive"], """
# Loot Fabricator

Turns a mob's Predictions into its drops. Put Predictions in, then click the drop to make. **Fixed** mode makes that
drop each time; **Queue** mode (the book) makes the drops of its queue in turn. Each drop uses one Prediction and
power.

A **Fabrication Directive** copies a fabricator's choices with a right-click, and gives them to another with a
sneaking right-click.

<RecipeFor id="loot_fabricator" />

<RecipeFor id="fab_directive" />
"""),
    "data_center.md": ("Data Center", 50, f"{ID}:data_center", [f"{ID}:data_center", f"{ID}:data_center_io_port"], """
# Data Center

Runs up to twenty-five **Self Aware** models at once into one set of outputs. It needs a hollow 7 by 7 by 7 shell:
an obsidian floor, black stained glass walls and ceiling, air inside. The controller replaces one block of the
lowest wall row, facing out.

Right-click a model's slot to choose what it makes. Each model takes one input per run.

**IO Ports** can replace any wall or ceiling block. Right-click one to switch it between Energy, Models, Inputs and
Outputs; pipes and cables then reach that part of the controller through it.

<GameScene zoom="2" interactive={true}>
  <ImportStructure src="scenes/data_center.snbt" />
</GameScene>

<RecipeFor id="data_center" />

<RecipeFor id="data_center_io_port" />
"""),
}

ES = {
    "index.md": ("Hostile Neural Networks", """
# Hostile Neural Networks

Convierte criaturas en datos, y los datos en sus botines, sin luchar contra ellas.

1. Haz clic derecho en una criatura con un **Marco de modelo** para sintonizar un [Modelo de datos](data_models.md).
2. Pon el modelo en un [Aprendiz profundo](deep_learner.md) y lucha contra esa criatura: cada muerte lo entrena.
3. Una [Cámara de simulación](sim_chamber.md) ejecuta el modelo con Matrices de predicción y energía.
4. Un [Fabricador de botín](loot_fabricator.md) convierte sus Predicciones en los botines que elijas.
5. Más tarde, un [Centro de datos](data_center.md) ejecuta veinticinco modelos Conscientes a la vez.

<RecipeFor id="blank_data_model" />

<RecipeFor id="prediction_matrix" />
"""),
    "data_models.md": ("Modelos de datos", """
# Modelos de datos

Un Modelo de datos guarda lo que se sabe de una criatura (y sus variantes). Tiene un **nivel**: Defectuoso, Básico,
Avanzado, Superior y Consciente. Cada nivel pide más datos y da más **precisión**, la probabilidad de que una
simulación haga la Predicción de la criatura. Un modelo Consciente siempre la hace.

Los datos vienen de matar a la criatura con el modelo en un Aprendiz profundo que lleves, o de una Cámara de
simulación en modo Entrenamiento. Un modelo Defectuoso no se puede simular: entrénalo hasta Básico.
"""),
    "deep_learner.md": ("Aprendiz profundo", """
# Aprendiz profundo

Guarda hasta cuatro Modelos de datos. Haz clic derecho con él para abrirlo; haz clic en un modelo de tu inventario
para meterlo, y en uno del aprendiz para sacarlo. Las flechas muestran cada modelo: su criatura, su nivel, su
precisión y cuántas muertes faltan para el siguiente nivel.

Llévalo en el inventario y cada muerte de la criatura de un modelo lo entrena.

<RecipeFor id="deep_learner" />
"""),
    "sim_chamber.md": ("Cámara de simulación", """
# Cámara de simulación

Pon un Modelo de datos en la ranura de la izquierda y Matrices de predicción en su entrada. Con energía (FE), cada
ejecución gasta una matriz y hace la **Predicción generalizada** del mundo y, según la precisión del modelo, la
**Predicción** de la criatura.

- El modo **Inferencia** (el ojo) produce. El modo **Entrenamiento** (la botella) da datos al modelo y gasta un
  quinto más de energía.
- El botón de redstone decide si una señal la enciende o la apaga.

<GameScene zoom="4" interactive={true}>
  <ImportStructure src="scenes/machines.snbt" />
</GameScene>

<RecipeFor id="sim_chamber" />
"""),
    "loot_fabricator.md": ("Fabricador de botín", """
# Fabricador de botín

Convierte las Predicciones de una criatura en sus botines. Mete Predicciones y haz clic en el botín que quieras. El
modo **Fijo** hace siempre ese botín; el modo **Cola** (el libro) hace por turno los de su cola.

Una **Directiva de fabricación** copia lo elegido en un fabricador con clic derecho, y lo aplica a otro agachado.

<RecipeFor id="loot_fabricator" />
"""),
    "data_center.md": ("Centro de datos", """
# Centro de datos

Ejecuta hasta veinticinco modelos **Conscientes** a la vez. Necesita una cáscara hueca de 7 por 7 por 7: suelo de
obsidiana, paredes y techo de cristal negro, aire dentro. El controlador sustituye un bloque de la fila más baja de
una pared, mirando hacia fuera.

Haz clic derecho en la ranura de un modelo para elegir qué hace. Los **Puertos de E/S** sustituyen cualquier bloque
de pared o techo; con clic derecho cambian entre Energía, Modelos, Entradas y Salidas.

<GameScene zoom="2" interactive={true}>
  <ImportStructure src="scenes/data_center.snbt" />
</GameScene>

<RecipeFor id="data_center" />
"""),
}


def front(title, position, icon, items, parent):
    lines = ["---", "navigation:", f"  title: {title}", f"  position: {position}", f"  icon: {icon}"]
    if parent:
        lines.append(f"  parent: {parent}")
    lines.append("item_ids:")
    lines += [f"- {i}" for i in items]
    lines.append("---")
    return "\n".join(lines) + "\n"


def write(files):
    base = f"assets/{ID}/ae2guide"
    for name, (title, pos, icon, items, body) in PAGES.items():
        parent = None if name == "index.md" else "index.md"
        files[f"{base}/{name}"] = (front(title, pos, icon, items, parent) + body.lstrip("\n")).encode()
        if name in ES:
            es_title, es_body = ES[name]
            files[f"{base}/_es_es/{name}"] = (front(es_title, pos, icon, [], parent) + es_body.lstrip("\n")).encode()
    files[f"{base}/guide.json"] = json.dumps({"title": f"{ID}.guide.title", "start": "index.md"}, indent=2).encode()
    north = "north"
    machines = [[0, 0, 0, f"{ID}:sim_chamber", {"facing": north}], [2, 0, 0, f"{ID}:loot_fabricator", {"facing": north}]]
    files[f"{base}/scenes/machines.json"] = json.dumps({"blocks": machines}).encode()
    shell = []
    for x in range(7):
        for y in range(7):
            for z in range(7):
                edge = x in (0, 6) or z in (0, 6) or y == 6
                if (x, y, z) == (3, 1, 0):
                    shell.append([x, y, z, f"{ID}:data_center", {"facing": north}])
                elif (x, y, z) == (6, 3, 3):
                    shell.append([x, y, z, f"{ID}:data_center_io_port", {"mode": "outputs"}])
                elif y == 0:
                    shell.append([x, y, z, "minecraft:obsidian"])
                elif edge:
                    shell.append([x, y, z, "minecraft:black_stained_glass"])
    files[f"{base}/scenes/data_center.json"] = json.dumps({"blocks": shell}).encode()
