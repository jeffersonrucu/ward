import sys,os;sys.dont_write_bytecode=True;sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from lib import *
# uso: python3 -I forms.py <arquivo.figcraft> [saida.json]  (padrão: forms.json ao lado do script)
d=load(sys.argv[1]);N=d['nodes']
OUT=sys.argv[2] if len(sys.argv)>2 else os.path.join(os.path.dirname(os.path.abspath(__file__)),'forms.json')
need=['Nome','Grupo','Ambiente','Projeto','Tags','Campos extras','Nível']
tipo={'login':['URL','Usuário','Senha','Semente TOTP'],'banco':['Motor','Host','Porta','Base','Usuário','Senha'],'servidor':['Host','Porta','Usuário','Senha','Chave privada','Passphrase','Servidor de salto (opcional)'],
'api':['URL base','Token','Nome da variável'],'aws':['Access key ID','Secret access key','Região','Session token (opcional)','Role ARN (opcional)'],'smtp':['Host','Porta','Usuário','Senha','Segurança'],'ftp/sftp':['Protocolo','Host','Porta','Usuário','Senha'],'nota segura':['Nota'],'genérico':[]}
res={}
for r in N['2796']['children']:
    nm=N[str(r)]['name']
    if not nm.startswith('Formulário /'): continue
    T=[];I=[]
    def fn(n,dep,anc):
        if n.get('visible',True) is False or any(a.get('visible',True) is False for a in anc): return
        if n.get('characters'): T.append(n['characters'])
        if n['type']=='instance':
            c=N[str(n['component'])];p=N[str(c['parent'])]
            I.append(p['name'] if p['type']=='componentSet' else c['name'])
    walk(N,r,fn)
    t=nm.split(' / ')[1]
    allt=' | '.join(T)
    falt=[x for x in need+tipo[t] if not any(s==x or s.startswith(x) for s in T)]
    res[nm]={'id':r,'faltando':falt,'anexos':('Anexos' in T),'campo extra(seg)':'segredo' in T,'TOTP':any('Código atual' in s for s in T),'Conectar':'Conectar' in T,'combobox':I.count('Select'),'Campo secreto':I.count('Campo secreto'),'Anexo':I.count('Anexo'),'Campo extra':I.count('Campo extra'),'Código TOTP':I.count('Código TOTP')}
for k in sorted(res): print(k,res[k])
json.dump(res,open(OUT,'w'),ensure_ascii=False,indent=1)
