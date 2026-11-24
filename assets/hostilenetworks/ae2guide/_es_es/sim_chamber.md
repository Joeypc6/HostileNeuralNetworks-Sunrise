---
navigation:
  title: Cámara de simulación
  position: 30
  icon: hostilenetworks:sim_chamber
  parent: index.md
item_ids:
---
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
