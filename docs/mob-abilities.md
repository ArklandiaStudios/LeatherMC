# Mob abilities reference

What every mob should eventually do, and which **system** each ability needs.
This is the roadmap for finishing mob behaviour (vanilla-accurate, from the
Minecraft Wiki). Status: `done`, `partial`, or `TODO (needs <system>)`.

Systems: **melee** (done), **ranged/projectiles** (in progress), **player health**
(done), **fly**, **swim/water** (needs worldgen water), **items/loot**,
**interaction** (tame/ride/breed/shear/milk/trade), **explosion**, **status
effects**, **summon**, **block-interaction** (pick up / place / climb).

## Attack types seen across mobs
melee · bow→arrow · crossbow→arrow · small fireball (blaze) · large fireball
(ghast) · splash potion (witch) · wind charge (breeze) · shulker bullet (homing)
· llama spit · wither skull · trident · evoker fangs + summon · guardian beam ·
warden sonic boom · dragon breath · touch damage (slime/magma) · explode (creeper).

## Hostile mobs

| Mob | Attack | Move | Special | Needs |
|-----|--------|------|---------|-------|
| zombie | melee | ground | breaks doors, sun-burn, reinforcements | items, day/night |
| husk | melee (hunger) | ground | no sun-burn | status effects |
| drowned | melee / trident | swim | throws trident | water, trident proj |
| zombie_villager | melee | ground | curable | interaction |
| skeleton | bow→arrow | ground | sun-burn | done (arrow) |
| stray | bow→arrow (slow) | ground | — | status effects |
| bogged | bow→arrow (poison) | ground | shearable | status, items |
| wither_skeleton | melee (wither) | ground | tall | status effects |
| creeper | none | ground | **approaches and explodes**; charged by lightning | explosion |
| spider | melee (poison) | climb | hostile at night | climb, day/night |
| cave_spider | melee (poison) | climb | small, fits 1-block | climb, status |
| silverfish | melee | ground | hides in blocks, summons | block-interaction, summon |
| endermite | melee | ground | small, despawns | — |
| slime | touch | hop | **splits on death** | hop, split |
| magma_cube | touch | hop | splits on death | hop, split |
| blaze | small fireball (3-burst) | **fly/hover** | fire-immune | fly (fireball done) |
| ghast | large fireball | **fly** | explosive shot | fly, explosion |
| phantom | melee swoop | **fly** | swoops at night | fly, swoop AI |
| vex | melee sword | **fly (thru walls)** | summoned, decays | fly, summon |
| witch | **splash potion** | ground | drinks buffs, potion-resistant | potion proj, status |
| evoker | **evoker fangs + summons vexes** | ground | no melee | summon, fangs |
| pillager | **crossbow→arrow** | ground | raids/patrols | arrow (crossbow) |
| illusioner | bow→arrow + blindness/clones | ground | unused | arrow, magic |
| vindicator | melee axe | ground | raids | — |
| ravager | melee + roar knockback | ground | raids, mounts | knockback aoe |
| breeze | **wind charge** | jumpy | deflects projectiles | wind_charge proj |
| shulker | **shulker bullet (homing)** | static | levitation, teleports, shell | homing proj, status |
| guardian | **beam laser** + thorns | swim | — | water, beam |
| elder_guardian | beam + thorns + mining fatigue | swim | — | water, beam, status |
| warden | melee huge + **sonic boom** | ground | blind, senses vibration | sonic boom, sculk |
| hoglin | melee knock-up | ground | fears warped fungus, breeds | interaction |
| zoglin | melee | ground | attacks everything | — |
| piglin_brute | melee axe | ground | — | — |
| giant | melee | ground | unused, huge | — |
| wither (boss) | **wither skulls** | fly | explodes, summons, wither effect | fly, skulls, explosion |
| ender_dragon (boss) | fireball + dragon breath + charge | fly | end crystals heal | fly, boss fight |
| creaking (26.2) | melee | ground | **frozen while looked at**, tied to heart | look-detection |
| parched (26.2) | TBD (husk-like?) | ground | new mob | research |
| sulfur_cube (26.2) | TBD (magma-like?) | hop? | new mob | research |

## Neutral mobs (attack only when provoked / conditional)

| Mob | Attack | Move | Special | Needs |
|-----|--------|------|---------|-------|
| wolf | melee | ground | **tameable (bones)**, packs, angry | tame |
| enderman | melee | ground | **teleports**, picks up blocks, water/look anger | teleport, blocks |
| bee | sting (poison, dies) | **fly** | pollinates, hive, group anger | fly, status |
| goat | ram (knockback) | ground | rams, jumps, drops horn | ram AI, items |
| llama / trader_llama | **spit** | ground | caravan, rideable+chest | spit proj, ride |
| iron_golem | melee + knock-up | ground | defends villagers | faction AI |
| polar_bear | melee | ground | defends cubs | — |
| panda | melee (rare) | ground | personalities, eats bamboo | AI variety |
| dolphin | melee | swim | leads to treasure, boosts player | water |
| piglin | **crossbow** + melee | ground | barters gold, hostile w/o gold armor | crossbow, barter |
| zombified_piglin | melee (gold sword) | ground | group anger | — |
| camel_husk (26.2) | TBD | ground | new mob | research |

## Passive mobs

| Mob | Move | Special | Needs |
|-----|------|---------|-------|
| pig | ground | rideable (saddle + carrot rod), breed carrots | ride, breed |
| cow / mooshroom | ground | milkable; mooshroom shearable→stew | interaction |
| sheep | ground | **eats grass→regrows wool**, shearable, dyeable | grass, items |
| chicken | ground | **lays eggs**, slow-falls | items, slow-fall |
| rabbit | **hop** | killer-bunny variant hostile | hop |
| frog | hop | eats slimes (tongue), lays frogspawn | items, water |
| horse / donkey / mule | ground | **tame + ride**; donkey/mule chest | tame, ride, inventory |
| camel | ground | ride (2 seats), dash, sit | ride |
| strider | **lava-walk** | ride (saddle + warped fungus), shivers on land | lava, ride |
| cat | ground | tame (fish), scares creepers, gifts | tame, items |
| ocelot | ground | gains trust, scares creepers | interaction |
| parrot | **fly** | tame, shoulder, imitates mobs, dances | fly, tame |
| fox | ground | hunts chickens/rabbits, holds items, sleeps | AI, items |
| bat | **fly** | erratic, sleeps on ceilings | fly |
| allay | **fly** | picks up & delivers items, duplicates | fly, items |
| armadillo | ground | rolls into ball (defense), drops scute | defense, items |
| sniffer | ground | sniffs & digs ancient seeds, lays egg | items |
| turtle | swim/ground | lays eggs on sand | water, items |
| axolotl | **swim** | attacks aquatic mobs, plays dead | water |
| cod / salmon / tropical_fish | **swim** | schools | water |
| pufferfish | swim | **inflates + poison** near threats | water, status |
| squid / glow_squid | swim | squirts ink | water |
| tadpole | swim | grows into frog | water |
| villager | ground | **trades**, professions, flees zombies, breeds | trading |
| wandering_trader | ground | trades, invis at night, despawns | trading |
| snow_golem | ground | **throws snowballs** (knockback), snow trail | snowball proj, blocks |
| happy_ghast (26.2) | **fly** | rideable flying mount | fly, ride |
| copper_golem (26.2) | ground | copper interactions, oxidizes | block-interaction, research |
| nautilus / zombie_nautilus (26.2) | swim? | new mobs | research |

## Implementation order (after this reference)
1. **Projectile variety** (now): witch potion, crossbow/bow arrows, breeze wind
   charge, shulker bullet, llama spit, wither skull — each ranged mob fires the
   right thing. *(self-contained)*
2. **Touch/contact** specials: slime/magma touch damage + hop + split on death,
   enderman teleport, creeper approach+explode. *(needs explosion for creeper)*
3. **Flight**: blaze/ghast/phantom/vex/bat/parrot/allay/bee/wither fly properly.
4. **Items/loot**: drops, chicken eggs, then interaction (tame/ride/breed/shear).
5. **Water** (after worldgen): all aquatic mobs, guardian beams, drowned trident.
6. **Bosses & complex**: warden sonic boom, evoker summon/fangs, wither,
   ender_dragon, and the new 26.2 mobs (research their behaviour).
