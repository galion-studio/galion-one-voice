"""No model loading/renders; test binary dispatch and off-path argument identity."""
import json,os,pathlib,subprocess,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
WRAPPER=ROOT/'bin/cbx-add-voice'
class Wrapper(unittest.TestCase):
    def setUp(self):
        # Keep test evidence; no automatic temp-dir deletion.
        self.work=pathlib.Path(tempfile.mkdtemp(prefix='blend-wrapper-',dir=None))
        self.log=self.work/'calls.jsonl'
        stub=self.work/'stub'
        stub.write_text('#!/usr/bin/env python3\nimport os,sys,json\nwith open(os.environ["CAPTURE"],"a") as f: f.write(json.dumps(sys.argv)+"\\n")\n')
        stub.chmod(0o755)
        self.env={**os.environ,'CAPTURE':str(self.log),'HF_HOME':str(self.work/'cache'),'CBX':str(stub),'CBX_BLEND':str(stub),'BLEND_REF':str(stub),'EMOTION_SCORER':str(stub)}
        self.args=['--name','own-unit-v2-blend-1','--voice-wav','original.wav','--dtype','fp32']
    def runw(self,*args): return subprocess.run([str(WRAPPER),*args],env=self.env,capture_output=True,text=True)
    def calls(self): return [json.loads(x) for x in self.log.read_text().splitlines()]
    def test_off_calls_only_existing_binary_with_unmodified_voice_args(self):
        r=self.runw(*self.args); self.assertEqual(r.returncode,0,r.stderr)
        calls=self.calls(); self.assertEqual(len(calls),1)
        self.assertEqual(calls[0][1:],['--intra-threads','2','--inter-threads','2','voice','add',*self.args])
    def test_explicit_flag_runs_builder_then_split_encode(self):
        r=self.runw('--emotion-blend','--blend-work-dir',str(self.work/'run'),*self.args)
        self.assertEqual(r.returncode,0,r.stderr); calls=self.calls(); self.assertEqual(len(calls),2)
        self.assertIn('--scorer',calls[0]); self.assertIn('--conditioning-wav',calls[1])
    def test_per_voice_opt_in_and_explicit_disable(self):
        cfg=self.work/'config.json'; cfg.write_text('{"emotion_blend":{"enabled":true}}')
        r=self.runw('--blend-config',str(cfg),'--no-emotion-blend',*self.args)
        self.assertEqual(r.returncode,0,r.stderr); self.assertEqual(len(self.calls()),1)
    def test_per_voice_enabled_dispatch(self):
        cfg=self.work/'config.json'; cfg.write_text('{"emotion_blend":{"enabled":true}}')
        r=self.runw('--blend-config',str(cfg),'--blend-work-dir',str(self.work/'run'),*self.args)
        self.assertEqual(r.returncode,0,r.stderr); self.assertEqual(len(self.calls()),2)
    def test_protected_deployment_fails_before_any_binary(self):
        r=self.runw('--name','own-samantha','--voice-wav','original.wav');self.assertNotEqual(r.returncode,0);self.assertFalse(self.log.exists())
    def test_existing_profile_fails_before_any_binary(self):
        cache=self.work/'cache/cbx/voices';cache.mkdir(parents=True);(cache/'own-unit-v2-blend-1.cbxvoice').write_bytes(b'KEEP')
        r=self.runw(*self.args); self.assertNotEqual(r.returncode,0);self.assertFalse(self.log.exists())
        self.assertEqual((cache/'own-unit-v2-blend-1.cbxvoice').read_bytes(),b'KEEP')
if __name__=='__main__': unittest.main()
