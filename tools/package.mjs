import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
const root=path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const destination=process.argv[2]?path.resolve(process.argv[2]):path.join(root,'dist','FanComposer');
const runtimeSource=path.join(root,'.venv','Lib','site-packages','openvino','libs');
const runtime=path.join(destination,'runtime');
await fs.mkdir(runtime,{recursive:true});await fs.mkdir(path.join(destination,'models'),{recursive:true});
const dlls=['openvino.dll','openvino_c.dll','openvino_ir_frontend.dll','openvino_intel_cpu_plugin.dll','openvino_intel_npu_plugin.dll','openvino_intel_npu_compiler.dll','openvino_intel_npu_compiler_loader.dll','openvino_intel_npu_vm_runtime.dll','tbb12.dll','tbbbind_2_5.dll','tbbmalloc.dll','tbbmalloc_proxy.dll'];
for(const dll of dlls)await fs.copyFile(path.join(runtimeSource,dll),path.join(runtime,dll));
for(const name of ['policy.xml','policy.bin','policy.json'])await fs.copyFile(path.join(root,'models',name),path.join(destination,'models',name));
await fs.cp(path.join(root,'.venv','Lib','site-packages','openvino-2026.4.1.dist-info','licenses'),path.join(runtime,'licenses'),{recursive:true});
await fs.copyFile(path.join(root,'target','release','fan-composer.exe'),path.join(destination,'fan-composer.exe'));
await fs.copyFile(path.join(root,'LICENSE'),path.join(destination,'LICENSE.txt'));
await fs.copyFile(path.join(root,'docs','PORTABLE_README.md'),path.join(destination,'README.md'));
const manifest={version:'0.3.0',created_utc:new Date().toISOString(),primary:'NPU',fallback:['OpenVINO CPU','CPU rules'],policy_hz:1,runtime:'OpenVINO 2026.4.1',files:[]};
for(const relative of ['fan-composer.exe',...dlls.map(v=>'runtime/'+v),'models/policy.xml','models/policy.bin','models/policy.json']){
 const file=await fs.readFile(path.join(destination,relative));
 manifest.files.push({path:relative,bytes:file.length,sha256:crypto.createHash('sha256').update(file).digest('hex')});
}
await fs.writeFile(path.join(destination,'manifest.json'),JSON.stringify(manifest,null,2));
console.log(JSON.stringify({destination,version:manifest.version,files:manifest.files.length,total_bytes:manifest.files.reduce((n,f)=>n+f.bytes,0)}));
