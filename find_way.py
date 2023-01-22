# Ref: ./map.md
from typing import *



MAP = [
	['*', 8, '-', 1],
	[4, '*', 11, '*'],
	['+', 4, '-', 18],
	['=22', '-', 9, '*'],
]
START = (3, 0)
END = (0, 3)

directions = [(1, 0, 'south'), (0, 1, 'east'), (-1, 0, 'north'), (0, -1, 'west')]

def update_val(prev_val: int, last_pos: Tuple[int, int], pos: Tuple[int, int]) -> int:
	op = MAP[pos[0]][pos[1]]
	if isinstance(op, str):
		if op[0] == '=':
			return 22
		else:
			return prev_val
	amount = op
	sign = MAP[last_pos[0]][last_pos[1]]
	return eval(f"{prev_val} {sign} {amount}")

def get_value(path: List[Tuple[int, int]]) -> int:
	current = 22
	for prev_pos, pos in zip(path, path[1:]):
		current = update_val(current, prev_pos, pos)
	return current

# def get_value(path: List[Tuple[int, int]]) -> int:
# 	current = 22
# 	eval_str = ""
# 	last_op = MAP[path[-1][0]][path[-1][1]]
# 	if isinstance(last_op, str):
# 		if last_op[0] == '=':
# 			return 22
# 		else:
# 			return get_value(path[:-1])

# 	for pos in path[1:]:
# 		eval_str += str(MAP[pos[0]][pos[1]]) + " "

# 	return eval(f"{current} {eval_str}")
		
# 	return current

def find_best_path():
	current = [(22, (START, None),)]
	while True:
		next_current = []
		for old_entry in current:
			old_value = old_entry[0]
			old_loc = old_entry[-1][0]
			for dx, dy, name in directions:
				new_x = dx + old_loc[0]
				new_y = dy + old_loc[1]
				new_pos = (new_x, new_y)
				if not (0 <= new_x < 4):
					continue
				if not (0 <= new_y < 4):
					continue
				if new_pos == START:
					continue
				# new_value = update_val(old_value, old_loc, (new_x, new_y))
				new_value = get_value([x for x, name in old_entry[1:]] + [(new_x, new_y)])
				new_entry = (new_value, *old_entry[1:], (new_pos, name))
				if new_pos == END and new_value == 30:
					print(new_entry)
					for a, name in new_entry[1:]:
						print(name, MAP[a[0]][a[1]])
					for a, name in new_entry[1:]:
						print(name)
					exit(0)
				elif new_pos == END:
					continue
				next_current.append(new_entry)

		current = next_current
find_best_path()
				


