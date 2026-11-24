---
navigation:
  title: "give_model"
  parent: index.md
  position: 1
  icon: hostilenetworks:data_model
level: 2
---
# give_model

**Uso:** `/hostilenetworks give_model <targets> <model> <tier> [data]`

Da a cada objetivo un modelo de datos ya entrenado hasta un nivel:

- `<targets>`: los jugadores que lo reciben;
- `<model>`: el id del modelo de datos, como `hostilenetworks:zombie`;
- `<tier>`: `faulty`, `basic`, `advanced`, `superior` o `self_aware`;
- `[data]`: datos extra sobre los que pide el nivel (0 por defecto).

Si el modelo no cabe en el inventario de un jugador, cae a sus pies, solo para él. Ponlo en una Cámara de Simulación para crear predicciones.

<GameScene zoom="4" interactive={true}>
  <Block id="hostilenetworks:sim_chamber" x="0" y="0" z="0" />
  <Block id="hostilenetworks:loot_fabricator" x="1" y="0" z="0" />
  <IsometricCamera yaw="195" pitch="30" />
</GameScene>

<Row>
  <ItemImage id="hostilenetworks:data_model" scale="2" />
  <ItemImage id="hostilenetworks:blank_data_model" scale="2" />
  <ItemImage id="hostilenetworks:deep_learner" scale="2" />
</Row>
