import unittest
import aggregate as a
class AggregationTests(unittest.TestCase):
 def test_ecapa_exact_subset(self):
  j={'scores':{'x/t01.wav':.8,'x/t02.wav':.6,'x/t03.wav':.1}}
  self.assertAlmostEqual(a.value('ecapa',j,['t01','t02']),.7)
 def test_missing_line_is_not_zero(self):
  self.assertIsNone(a.value('ecapa',{'scores':{'t01.wav':.7}},['t01','t02']))
 def test_artic_is_subset_not_global_distribution(self):
  j={'files':[{'file':'t01.wav','rate':{'articulation_rate_syl_s':2}}, {'file':'t02.wav','rate':{'articulation_rate_syl_s':8}}], 'distribution':{'articulation_rate_syl_s':{'mean':5}}}
  self.assertEqual(a.value('artic',j,['t01']),2)
 def test_known_validation12(self):
  ids=[f't{i:02}' for i in range(1,13)]
  self.assertAlmostEqual(a.value('emotion',a.read(a.V/'emotion-fix/scores/sd_r2_t012_emo.json'),ids),.4674980541070302)
  self.assertAlmostEqual(a.value('ecapa',a.read(a.V/'emotion-fix/scores/sd_r2_t012_ecapa.json'),ids),.7082,places=4)
if __name__=='__main__':unittest.main()
