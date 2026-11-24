---
navigation:
  title: Simulation Chamber
  position: 30
  icon: hostilenetworks:sim_chamber
  parent: index.md
item_ids:
- hostilenetworks:sim_chamber
- hostilenetworks:prediction
- hostilenetworks:overworld_prediction
- hostilenetworks:nether_prediction
- hostilenetworks:end_prediction
- hostilenetworks:twilight_prediction
---
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
