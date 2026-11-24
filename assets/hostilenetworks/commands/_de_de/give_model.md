---
navigation:
  title: "give_model"
  parent: index.md
  position: 1
  icon: hostilenetworks:data_model
level: 2
---
# give_model

**Verwendung:** `/hostilenetworks give_model <targets> <model> <tier> [data]`

Gibt jedem Ziel ein Datenmodell, das schon bis zu einer Stufe trainiert ist:

- `<targets>`: die Spieler, die es bekommen;
- `<model>`: die ID des Datenmodells, etwa `hostilenetworks:zombie`;
- `<tier>`: `faulty`, `basic`, `advanced`, `superior` oder `self_aware`;
- `[data]`: zusätzliche Daten über das hinaus, was die Stufe braucht (standardmäßig 0).

Passt das Modell nicht ins Inventar eines Spielers, fällt es ihm vor die Füße, nur für ihn. Leg es in eine Simulationskammer, um Vorhersagen zu erzeugen.

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
