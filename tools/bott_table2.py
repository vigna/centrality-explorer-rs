"""Table 2, "Social responses among eleven children under five categories"
(p. 62), of E. A. Bott, W. E. Blatz, N. Chant and H. Bott, "Observation and
training of fundamental habits in young children", Genetic Psychology
Monographs 4(1), July 1928, transcribed by hand from a scan of the printed
page.

Every row and column category total printed in the table matches the cells
below (run this file to check). Three printed overall totals disagree with
their own category totals and are misprints: row D (53 for 59), column M (188
for 191) and the grand total (855 for 835). Some children are starred in the
table (F, H, J, N, O); the star is not explained on that page.
"""
# T[row][col] = [talking, interference, watching, imitation, cooperation]:
# the behaviour of the row child toward the column child. Missing cells are 0.
KIDS = ["A", "D", "E", "F", "G", "H", "J", "K", "M", "N", "O"]   # F,H,J,N,O are starred in the table
AGES = [26, 29, 31, 36, 37, 38, 40, 45, 52, 53, 54]               # months
T = {
 "A": {"D":[5,0,0,1,0], "E":[2,3,1,1,0], "F":[1,1,1,0,0], "J":[0,1,1,0,0], "K":[15,5,1,3,3],
       "M":[1,0,0,0,0], "N":[0,1,0,0,0], "O":[0,1,1,0,0]},
 "D": {"A":[4,0,1,2,0], "E":[2,1,1,3,1], "F":[1,1,3,0,1], "G":[0,0,0,2,0], "H":[2,2,1,0,2],
       "J":[0,0,1,0,0], "K":[1,2,5,2,4], "M":[1,1,5,0,0], "N":[0,1,0,0,0], "O":[0,0,6,0,0]},
 "E": {"A":[3,6,0,0,0], "D":[4,7,2,1,0], "F":[3,12,0,1,0], "G":[0,1,0,0,0], "H":[0,2,0,1,0],
       "J":[1,5,1,3,0], "K":[4,7,1,0,0], "M":[4,5,3,3,0], "N":[1,7,0,1,1], "O":[4,5,2,2,1]},
 "F": {"D":[0,1,1,0,0], "E":[3,2,1,0,0], "G":[2,2,0,2,0], "H":[0,0,0,1,0], "J":[2,0,0,1,0],
       "M":[3,3,2,0,0], "N":[2,0,0,2,0], "O":[11,2,6,3,1]},
 "G": {"A":[2,0,0,0,0], "D":[0,2,2,0,0], "E":[1,1,0,0,0], "F":[1,0,1,1,0], "H":[2,3,0,0,0],
       "J":[0,0,1,0,0], "K":[5,2,2,0,2], "M":[3,5,1,2,2], "N":[0,4,0,0,0], "O":[2,4,3,0,1]},
 "H": {"A":[0,0,1,0,0], "D":[2,2,2,0,1], "E":[0,3,2,0,0], "F":[0,0,3,0,0], "G":[0,3,3,0,0],
       "K":[0,0,5,1,0], "M":[0,1,4,1,0], "N":[1,0,4,0,0],
       "O":[0,0,0,0,1]},   # a faint mark at the cell edge; both the H row and O column totals need it
 "J": {"A":[4,3,1,0,0], "D":[0,1,0,0,0], "E":[16,6,0,0,0], "F":[0,2,0,0,0], "K":[1,2,1,0,0],
       "M":[20,3,2,0,0], "N":[7,1,2,1,0], "O":[9,0,3,1,0]},
 "K": {"A":[5,2,0,1,1], "D":[2,1,0,0,0], "E":[2,3,0,0,0], "F":[3,1,0,0,0], "G":[4,2,0,0,3],
       "J":[1,2,0,0,0], "M":[20,6,0,0,5], "N":[2,1,0,0,1], "O":[3,1,0,1,0]},
 "M": {"D":[3,1,0,0,0], "E":[3,0,0,0,0], "F":[1,2,0,1,0], "G":[2,0,0,1,0], "J":[0,0,0,1,0],
       "K":[8,1,0,0,1], "N":[3,0,0,0,0], "O":[7,3,4,18,9]},
 "N": {"A":[2,4,5,0,0], "D":[1,2,2,0,0], "E":[3,4,0,0,0], "F":[5,8,1,0,0], "G":[10,2,0,0,1],
       "H":[1,0,0,0,0], "J":[10,0,0,0,1], "K":[8,0,0,1,0], "M":[6,0,1,1,0], "O":[18,2,3,2,0]},
 "O": {"A":[1,1,1,0,0], "D":[1,1,0,0,1], "E":[4,0,2,0,0], "F":[11,1,1,0,0], "G":[0,2,0,0,0],
       "H":[1,0,0,0,0], "J":[4,1,0,0,0], "K":[3,2,0,0,0], "M":[65,7,1,1,3], "N":[25,3,0,0,0]},
}
# Printed totals: per row child (behaviour shown) and per column child (responses received).
# Every row and column category total matches the cells below. Three printed overall totals
# disagree with their own category totals: row D prints 53 for 11+8+23+9+8 = 59, column M
# prints 188 for 123+31+19+8+10 = 191, and the grand total prints 855 for 835.
ROW_TOTALS = {"A":[24,12,5,5,3], "D":[11,8,23,9,8], "E":[24,57,9,12,2], "F":[23,10,10,9,1],
  "G":[16,21,10,3,5], "H":[3,9,24,2,2], "J":[57,18,9,2,0], "K":[42,19,0,2,10],
  "M":[27,7,4,21,10], "N":[64,22,12,4,2], "O":[115,18,5,1,4]}
COL_TOTALS = {"A":[21,16,9,3,1], "D":[18,18,9,2,2], "E":[36,23,7,4,1], "F":[26,28,10,3,1],
  "G":[18,12,3,5,4], "H":[6,7,1,2,2], "J":[18,9,4,5,1], "K":[45,21,15,7,10],
  "M":[123,31,19,8,10], "N":[41,18,6,4,2], "O":[54,18,28,27,13]}

if __name__ == "__main__":
    cats = ["talk", "int", "watch", "imit", "coop"]
    for r in KIDS:
        s = [sum(T[r].get(c, [0]*5)[k] for c in KIDS) for k in range(5)]
        if s != ROW_TOTALS[r]: print("ROW", r, "transcribed", s, "printed", ROW_TOTALS[r])
    for c in KIDS:
        s = [sum(T[r].get(c, [0]*5)[k] for r in KIDS) for k in range(5)]
        if s != COL_TOTALS[c]:
            print("COL", c, "transcribed", s, "printed", COL_TOTALS[c],
                  "diff", {cats[k]: COL_TOTALS[c][k] - s[k] for k in range(5) if s[k] != COL_TOTALS[c][k]})
    print("grand", [sum(v[k] for r in T for v in T[r].values()) for k in range(5)], "printed", [406,201,111,70,47])
