---
navigation:
  title: Data Center
  position: 50
  icon: hostilenetworks:data_center
  parent: index.md
item_ids:
- hostilenetworks:data_center
- hostilenetworks:data_center_io_port
---
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
