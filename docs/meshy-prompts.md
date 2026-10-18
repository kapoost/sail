# Prompty do Meshy — kadłub jachtu i kadłub brygu

Do sprawdzenia, czy generator daje kadłub lepszy od tego, co mamy: jacht jedzie dziś na
`venus.glb` (skanie przeskalowanym do LOA), bryg jest w całości proceduralny w
`crates/sail-app/src/ship.rs`.

**Zasada, która trzyma oba prompty:** z Meshy ma przyjść **kadłub i pokład, nic więcej**.
Maszty, reje, liny i żagle muszą się ruszać, więc są rysowane w kodzie i nie mogą być
częścią statycznej siatki. Generator i tak nie zrobi want ani wyblinek — struktury
cieńsze niż jego siatka wychodzą jako roztopiony makaron i zabierają ze sobą budżet
trójkątów.

---

## 1. Jacht — „archipelago packet 36"

```
An 11-metre wooden sailing yacht hull, fractional sloop, Nordic archipelago
daysailer. Long low sheer, fine entry, slightly raked stem, narrow tucked
transom, deep fin keel and spade rudder. Varnished mahogany topsides over a
dark green boot top, teak deck, low cabin trunk with small bronze portlights,
deep open cockpit with a tiller, bronze winches and cleats on the side decks.
Bare mast stump at the deck, no sails, no rigging. Realistic PBR game asset,
single object, centred, no water, no base, no background.
```

**Negative prompt**

```
sails, rigging, ropes, shrouds, flags, water, waves, ocean, dock, pier, people,
text, logo, base plate, pedestal, toy, cartoon
```

**Wymiary do sprawdzenia po eksporcie** (z `sail_core::boat::archipelago_packet`):
LOA 11,00 m · LWL 9,50 m · szerokość 3,40 m · zanurzenie 2,25 m · wyporność 3600 kg.
Maszt stoi 17,8 m nad pokładem, bom 2,62 m — oba rysuje `rig.rs`.

---

## 2. Bryg — żaglowiec z misji pilotażu

```
A 31-metre two-masted square-rigged brig, 1850s coasting trader, bare poles.
Black topsides with a broad ochre painted band and painted gunports along it,
copper-sheathed bottom below the waterline, heavy bulwarks with pin rails, a
raised quarterdeck and a flat transom with a small stern window. Long steeved
bowsprit. Two masts, each in three pieces — lower mast, topmast, topgallant —
with a fighting top platform at each lower masthead, and three bare yards on
each mast. Plank deck with cargo hatches, windlass, ship's boat on chocks.
No sails, no rigging ropes. Realistic PBR game asset, single object, no water,
no background.
```

**Negative prompt**

```
sails, canvas, ropes, ratlines, shrouds, stays, rigging lines, flags, water,
waves, harbour, people, text, base plate, cartoon, toy
```

**Wymiary** (z `ship.rs`): kadłub 31 m · szerokość 8,2 m · zanurzenie 3,9 m · wolna burta
2,3 m · nadburcie 1,25 m. Maszty: fok w 26 % długości od dziobu, grot w 63 %; dolny
15,5 m, stenga 11,5 m, bramstenga 7,5 m. Bukszpryt 9 m. Reje 10,2 / 8,5 / 6,2 m.

---

## Ustawienia i co sprawdzić

**Generowanie.** Text to 3D → Preview w stylu *Realistic*, 3–4 warianty; refine tylko
najlepszego, z PBR. Preview już pokazuje, czy kształt kadłuba jest kadłubem — refine
poprawia teksturę, nie proporcje.

**Po pobraniu `.glb`:**

1. **Skala.** Meshy eksportuje w dowolnych jednostkach. `venus.glb` też skalujemy do LOA
   — patrz stała `SCAN_LOA` w `boat.rs`.
2. **Oś dziobu.** U nas dziób to −Z, prawa burta +X, góra +Y (`frame.rs`).
3. **Szczelność.** Czy dno jest zamknięte i czy nie ma ścianek wewnątrz — przy przekroju
   kamerą widać od razu.
4. **Trójkąty.** Morze zjada 40 tys. wierzchołków na klatkę, więc dla brygu cel to
   25–40 tys. trójkątów, dla jachtu mniej.

**Czego nie da:** malowania dokładnie w pasie wzdłużnika, równej grubości nadburcia i
pawęży z podcięciem. Jeśli kadłub z Meshy będzie lepszy od proceduralnego — a rufa brygu
jest dziś za bardzo pudełkowata — podmieniamy sam kadłub i zostawiamy omasztowanie z
kodu.
