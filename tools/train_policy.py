"""Local rule-teacher distillation. Not trained on comfort ratings or claimed superior to rules."""
import os
os.environ["OPENBLAS_NUM_THREADS"]="1"
os.environ["OMP_NUM_THREADS"]="1"
import json
from pathlib import Path
import numpy as np
import openvino as ov
from openvino import opset13 as op
rng=np.random.default_rng(255)
def teacher(x):
    pitch,level,tonal,centroid,flat,broad,dp,dl,confidence,ts,tc,prev_b,prev_d,prev_m,mode,pref=x.T
    return np.stack([
        .25+.30*centroid+.12*flat+.08*mode-.10*pref+.12*prev_b,
        .20+.28*tonal-.13*flat-.10*dp+.14*mode+.12*prev_d,
        .32+.30*tonal+.12*confidence-.15*flat+.05*tc,
        .30+.18*ts-.10*dl+.22*prev_m+.08*mode,
        .62-.16*level+.12*flat+.06*mode,
        .60+.20*mode-.15*level+.10*broad
    ],axis=1).clip(.05,.95).astype(np.float32)
x=rng.uniform(0,1,(32768,16)).astype(np.float32);y=teacher(x)
test=rng.uniform(0,1,(4096,16)).astype(np.float32);expected=teacher(test)
w1=rng.normal(0,.24,(16,32)).astype(np.float32);b1=np.zeros(32,np.float32)
w2=rng.normal(0,.12,(32,6)).astype(np.float32);b2=np.zeros(6,np.float32)
params=[w1,b1,w2,b2];m=[np.zeros_like(p)for p in params];v=[np.zeros_like(p)for p in params]
def forward(a):
    h=np.maximum(a@w1+b1,0);out=1/(1+np.exp(-(h@w2+b2)));return h,out
for step in range(1,1201):
    idx=rng.integers(0,len(x),1024);a=x[idx];truth=y[idx]
    h,out=forward(a);dz=2*(out-truth)*out*(1-out)/(len(a)*6)
    g2=h.T@dz;gb2=dz.sum(0);dh=(dz@w2.T)*(h>0);g1=a.T@dh;gb1=dh.sum(0)
    for i,(p,g) in enumerate(zip(params,[g1,gb1,g2,gb2])):
        m[i]*=.9;m[i]+=.1*g;v[i]*=.999;v[i]+=.001*g*g
        p-=.008*(m[i]/(1-.9**step))/(np.sqrt(v[i]/(1-.999**step))+1e-8)
_,pred=forward(test);mae=float(np.mean(np.abs(pred-expected)));maximum=float(np.max(np.abs(pred-expected)))
assert mae<.012,(mae,maximum)
feature=op.parameter([1,16],np.float32,name="fan_music_features")
hidden=op.relu(op.add(op.matmul(feature,op.constant(w1),False,False),op.constant(b1)))
output=op.sigmoid(op.add(op.matmul(hidden,op.constant(w2),False,False),op.constant(b2)))
model=ov.Model([output],[feature],"fan_music_teacher_distilled_v1")
Path("models").mkdir(exist_ok=True);ov.save_model(model,"models/policy.xml",compress_to_fp16=False)
metadata={"name":"teacher-distilled-policy-v1","training":"supervised imitation of documented conservative rules; no human comfort training","seed":255,"shape":[1,16],"output_shape":[1,6],"hidden":32,"steps":1200,"train_samples":32768,"holdout_samples":4096,"holdout_mae":mae,"holdout_max_error":maximum,"runtime":ov.__version__,"features":["pitch","level","tonality","centroid","flatness","broadband","pitch_rate","level_rate","confidence","time_sin","time_cos","previous_brightness","previous_density","previous_motion","mode","preference"],"outputs":["brightness","density","overtones","motion","reverb","stereo"],"quality_improvement_claim":False}
Path("models/policy.json").write_text(json.dumps(metadata,indent=2),encoding="utf-8")
print(json.dumps(metadata,indent=2))
