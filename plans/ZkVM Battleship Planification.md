
- The grid can be imagined in two ways : 
	- A matrix (that contains 10x10 case) and where we should have flags for those who are occupied, flags that would turn down if they have been hit.
	- No "concrete" representation of the grid, but each player has ships, and ships has a field that denotes the coordination hey occupy and wither this coordinate has been hit or not.

==Little idea : we can now work on the CLI part where the two players play in one PC, later on we can extend what is happening to multiplayer (on the blockchain ?)==

# 1. Game Rules : 

- We have **two** participants that should have two (10x10) grid, one where he will place his ships and one where he will use it to hit "the opponent".
	- The placement of ships should be validated, and should not be allowed to change. (*This must be part of the game rules, not of the proving system)
- Everyone has **5 ships** 
	- The carrier (5 squares)
	- Battleship (4 squares)
	- Cruiser (3 squares)
	- Submarine (3 squares)
	- Destroyer (2 squares)
- The initial placement should be **vertical OR horizontal** (not diagonal), and all the ship should be included **inside of the grid**, and **none should be overlapping**.

- The first who should start call a shot on the grid grid of shots, and the opponent say if it is a hit or a miss
	- If it is a hit, a red flag is raised in the place.
	- If it is a miss a yellow one is raised in the place.
- If a ship is hit on all its presence on the board, it sink. The type of the ship sunk **must be announced**.
- He who sink the entire fleet will win.


- Hidden state : 
	- The position of the ships (Grid map) is **static** from start to finish, and this should be enforced by the rules of the game.
	- the public transcript of shots and results evolves publicly.
	- the defender’s private occupancy state relative to prior hits evolves logically as hits accumulate.


# 2. Disclosure Policy  :

- What is a secret : 
	- The relationship of "ship location <-> type", **until a ship sink**. In other words, the type attached to a still-hidden cluster is secret, in addition to the "initial" presence of a ship there.

- What should be "proved" : 
	- ==The correctness of the hit/miss. ==
		- The **receiving player** should prove that the attack has either hit a target, or missed it.
	- ==That a ship of a given type sunk.==
		- *Question* : how to incite/oblige the defender to prove that his ship sunk.

- What should happen after each round ? (starting from round 1, as round 0 is for the placement and validation of the grid)
	- The attacker will **initiate** the attack, calling it on a specific coordinate.
	- The defender -holder of the secret state- will receive the attack, and position it on his grid
		- *Question* : should the defender "change" his state, or create a parallel grid/state where he could map the attacks, or what?
	- the defender is then a **prover** who will prove that the attack either **missed or hit**.
	- The **main verifier** is the attacker, but anyone (observers) can verify the claim of the defender.
	- If a ship sunk :
		- The defender will have to prove that **a specific ship with a specific type sunk**, because it is a rule, Immediately after it sinks.

- After each one round, the facts that should become knwon are : 
	- The position that the attack was directed to.


>[!Note]
>Inferring the type of ships that remains is allowed for the context of this game, it should not however alter the secrecy of "where an exact ship is positioned".
>So, the "structure" deduced from the hits is acceptable.

- The replay attacks AND error message leakages will be treated in the proof system mechanics
- the observer is an attacker, he shouldn't see either boards, only the : 
	- Attacks
	- Ships sunk
	- Declaration of hit/miss and their proofs.


# 3. Proof Systems mechanics : 

