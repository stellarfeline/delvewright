# What a finished journal entry says — established practice

For the engine developer deciding what the journal writes once an objective is over. It answers one question: when a task can end more than one way, what does an established quest journal keep of it, and who decides which words are shown. The rules built on it are in spec-0137; this page holds only what was found.

**The gap this fills.** `game-writing.md` §8 L1 says what the journal tells of an *open* objective and stops there. Spec-0104 §2.5 cites that a journal "must keep old information available" and gives a done objective its struck title. Neither says what a finished entry holds.

## 1. What was read

| # | Source | Read how |
|---|---|---|
| S1 | The Oblivion Construction Set Wiki, *Quest Stages Tab* (`cs.uesp.net/wiki/Quest_Stages_Tab`, revision 18391) — the quest editor's own reference | the page text, whole |
| S2 | The Unofficial Elder Scrolls Pages, *Oblivion:Journal* (`en.uesp.net/wiki/Oblivion:Journal`, revision 2917864) | the page text, whole |
| S3 | The Unofficial Elder Scrolls Pages, *Skyrim:In My Time Of Need*, the quest stages table (`en.uesp.net/wiki/Skyrim:In_My_Time_Of_Need`, revision 3643958) — one shipped quest with two endings | the table and its notes |
| S4 | G. Lobanov, *Game Musings: Quest Logs* (2023, `greglobanov.substack.com/p/game-musings-quest-logs`) | **through a page summary only**; its one quotation below is relayed, not read in the original |

S1–S3 are wiki text under CC BY-SA; they are quoted in short excerpts for citation and nothing of them is adopted. All four describe single-player games with a menu journal. Nothing read describes a journal shared by a party, or one written into a held item.

## 2. What they say

**The story writes the entry, at the moment it decides.** S1: "Setting a quest stage (see SetStage) adds the Log Entry to the player's quest log and runs the Result Script." A stage is set by the script of the dialogue answer or the event that reached it, so the words enter the journal from the place the story turned, and the journal never works out afterwards what happened.

**One line per way, and conditions pick it.** S1: "Each stage can have 1 or more 'stage items', which can include a log entry and/or a result script. Multiple stage items are often conditionalized so that only one is actually written to the player's log when a stage is set." S3 shows the result in a shipped quest: stage 200 and stage 201 are both marked "Finishes quest", and each carries its own entry — one for the party who protected the fugitive, one for the party who handed her over. S3's notes: "Not all Journal Entries may appear in your journal; which entries appear and which entries do not depends on the manner in which the quest is done."

**The finishing entry states the deed, in the past, in a sentence or two.** S3, stage 201: "After making arrangements with Kematu, I've delivered Saadia to him. He assures me that she will be taken back to Hammerfell alive, though after that her fate is no longer in his hands." It names what was done and to whom; it does not restate the instructions.

**A finished quest moves to its own list, and shows its latest entry.** S1: "Complete Quest: If this box is checked, setting this stage will move the quest from the active to completed portions of the player's journal." S2: "Completed Quests: Lists the most recent quest entries for each of the quests you have completed." S3's notes add that once a quest is finished "you may still receive new entries for that quest".

**A failed quest is kept too.** S3 lists a stage marked "Fails quest" beside the two that finish it: the journal keeps a task that ended without being done, and says so.

**A log keeps the past.** S4, relayed: a quest log "often … will keep a record of past finished quests too."

## 3. What they do not say

- Nothing read gives a rule for a task **the player was never told about** and did anyway. S3's "which entries appear … depends on the manner in which the quest is done" is the nearest statement, and it is about entries inside a known quest.
- Nothing read fixes the **grammatical person** of an entry for more than one player; S3's entries are first person singular because the game has one player.
- Nothing read measures whether players **read** finished entries. No claim about that is made here or in the spec.
- The quest editor in S1 keeps an entry's words with the quest, in a table of stages, and the story points at a stage by number. Whether the words sit with the quest or with the event that writes them is a property of that editor's file format, not a finding about journals.
