---
navigation:
  title: "give_model"
  parent: index.md
  position: 1
  icon: hostilenetworks:data_model
level: 2
---
# give_model

**Usage:** `/hostilenetworks give_model <targets> <model> <tier> [data]`

Gives each target a data model already trained to a tier:

- `<targets>`: the players who get it;
- `<model>`: the data model's id, such as `hostilenetworks:zombie`;
- `<tier>`: `faulty`, `basic`, `advanced`, `superior` or `self_aware`;
- `[data]`: extra data on top of what the tier needs (0 by default).

A model that does not fit in a player's inventory drops at their feet, for them alone. Put it in a Simulation Chamber to make predictions.

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
