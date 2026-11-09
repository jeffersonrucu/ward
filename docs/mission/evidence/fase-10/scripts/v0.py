import sys,os,re,base64,urllib.parse;sys.dont_write_bytecode=True;sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
# uso: python3 -I v0.py <arquivo.figcraft> <design-nodes.md> <saida.json>
from lib import *
from collections import Counter
F=sys.argv[1]; MD=sys.argv[2]; OUT=sys.argv[3]
d=load(F);N=d['nodes'];V=d['variables']
L=N['2796']['children']; top={N[str(c)]['name']:c for c in L}
R={}
# V0.1
cols={c['id']:c for c in V['collections']}
byname={c['name']:c for c in V['collections']}
cores=[v['name'] for v in V['variables'] if v['collection']==byname['Cores']['id']]
cats=['fundo','superficie','texto','borda','primaria','perigo','aviso','sucesso','ambiente/dev','ambiente/stage','ambiente/prod']
raio={v['name']:list(v['values'].values())[0] for v in V['variables'] if v['collection']==byname['Raio']['id']}
R['V0.1']={'colecoes':[c['name'] for c in V['collections']],'modos_Cores':[m['name'] for m in byname['Cores']['modes']],
  'tem_Tipografia_Espacamento_Raio_Largura':all(k in byname for k in ['Tipografia','Espaçamento','Raio','Largura']),
  'semanticas_cobertas':{c:any(n.startswith(c) for n in cores) for c in cats},'raio':{k:(v.get('value')) for k,v in raio.items()}}
# V0.2 solto (fora de instância) + overrides soltos
iss=[]
def chk(n,dep,anc):
    if any(a['type']=='instance' for a in anc) or n.get('visible') is False: 
        o=n.get('overrides') or [];b=n.get('bound') or {}
        if 'fills' in o and any(f.get('visible',True) for f in n.get('fills') or []) and not any(k.startswith('fill') for k in b): iss.append((n['id'],'override fill'))
        if 'strokes' in o and n.get('strokes') and not any(k.startswith('stroke') for k in b): iss.append((n['id'],'override stroke'))
        if 'corners' in o and not any('adius' in k for k in b): iss.append((n['id'],'override raio'))
        return
    b=n.get('bound') or {}
    if any(f.get('visible',True) for f in n.get('fills') or []) and not any(k.startswith('fill') for k in b): iss.append((n['id'],'fill'))
    if n.get('strokes') and not any(k.startswith('stroke') for k in b): iss.append((n['id'],'stroke'))
    r=(n.get('corners') or {}).get('radii')
    if r and any(r) and not any('adius' in k for k in b): iss.append((n['id'],'raio'))
    Lo=n.get('layout')
    if Lo and Lo.get('mode') not in (None,'none'):
        if Lo.get('gap') and not any(k in b for k in ('gap','itemSpacing')) and Lo.get('main_align')!='spaceBetween': iss.append((n['id'],'gap'))
        if any(Lo.get('padding') or [0]) and not any(k.startswith('padding') for k in b): iss.append((n['id'],'padding'))
    if n['type']=='text' and not (n.get('style') or {}).get('style_id'): iss.append((n['id'],'texto sem estilo'))
for r in L: walk(N,r,chk)
R['V0.2']={'frames':len(L),'valores_soltos':iss}
# V0.3
comps={}
for k,n in N.items():
    if n['type']=='componentSet': comps[n['name']]={'id':int(k),'variantes':[N[str(c)]['name'] for c in n['children'] if N[str(c)]['type']=='component']}
for k,n in N.items():
    if n['type']=='component' and N[str(n['parent'])]['type']!='componentSet': comps.setdefault(n['name'],{'id':int(k),'variantes':[]})
def pg(i):
    n=N[str(i)]
    while n['type']!='page': n=N[str(n['parent'])]
    return n['name']
R['V0.3']={k:comps.get(k) for k in ['Botão','Campo','Campo secreto','Select','Badge ambiente','Badge tipo','Item de lista','Modal','Toast','Campo PIN','Campo senha','Botão ícone']}
R['V0.3']['componentes_fora_da_Design_System']=[k for k,o in comps.items() if pg(o['id'])!='Design System']
# V0.4 desanexados: frames soltos com cara de componente
susp=[];kinds=Counter()
compnames=set(comps)
def c4(n,dep,anc):
    if any(a['type']=='instance' for a in anc): return
    if n['type']=='instance': kinds[N[str(N[str(n['component'])]['parent'])]['name'] if N[str(N[str(n['component'])]['parent'])]['type']=='componentSet' else N[str(n['component'])]['name']]+=1;return
    if dep>0 and (n['name'] in compnames or n['name'].lower() in ('fechar','botão','campo','select','badge','toast','modal')): susp.append((n['id'],n['name']))
for r in L: walk(N,r,c4)
R['V0.4']={'suspeitos_desanexados':susp,'instancias_por_componente':dict(kinds.most_common()),'instancias_com_componente_ausente':[k for k,n in N.items() if n['type']=='instance' and str(n.get('component')) not in N]}
# V0.5-V0.18 existência
need=['Primeiro uso / Criar cofre','Primeiro uso / Senha mestra','Primeiro uso / PIN','Destravar / Padrão','Destravar / Senha errada','Destravar / Pedido do agente','Credenciais / Vazia','Credenciais / Cheia','Credenciais / Filtrada','Nível de acesso / Escolher','Nível de acesso / PIN','Nível de acesso / PIN errado','Gerador de senha / Avulso','Gerador de senha / No formulário','Configurações / Geral','Configurações / Backup externo ligado','Restaurar backup / Versões','Restaurar backup / Confirmar','Kit de emergência / Impressão','Aprovação / Normal','Aprovação / prod','Aprovação / PIN errado','Aprovação / PIN bloqueado','Aprovação / Expirando','Bandeja / Destravado','Bandeja / Travado','Extensão / Sugestões do domínio','Extensão / Salvar login novo','Extensão / Cofre travado']
for t in ['login','banco','servidor','api','aws','smtp','ftp/sftp','nota segura','genérico']:
    need+=[f'Formulário / {t} / Novo',f'Formulário / {t} / Edição']
R['V0.5-18_frames']={k:({'id':top[k],'w':N[str(top[k])]['width'],'h':N[str(top[k])]['height']} if k in top else None) for k in need}
R['V0.5-18_faltando']=[k for k in need if k not in top]
# V0.19
modes=[]
for pid in ('2','2796'):
    for r in N[pid]['children']: walk(N,r,lambda n,a,b: modes.append((n['id'],n['var_modes'])) if (n.get('var_modes') or {}).get('6:54') not in (None,'6:2') else None)
txt=set()
for r in L: walk(N,r,lambda n,a,b: txt.add(n['characters']) if n.get('characters') else None)
R['V0.19']={'modo_paginas':{N[p]['name']:N[p].get('var_modes') for p in ('2','2796')},'nodes_com_Cores_diferente_de_Escuro':modes,
  'textos_com_tema_claro':[t for t in txt if re.search(r'\b(tema|claro|modo escuro|modo claro)\b',t,re.I)],
  'componentes_com_claro':[k for k in comps if re.search('claro|light',k,re.I)]}
# V0.20
# A saída registra só o NOME de cada valor procurado e se foi achado, nunca o literal
# (S15: evidência não repete valor secreto, nem falso; o gitleaks acusa o literal).
seg={'senha-mestra':'falso-mestra-ward-01','pin':'135790','acme-wp-dev.senha':'falso-WP-senha-01',
 'acme-wp-prod.senha':'falso-WP-senha-02','acme-wp.totp':'JBSWY3DPEHPK3PXP','acme-pg.senha':'falso-PG:s3nha/01@#',
 'acme-ssh-senha-stage.senha':'falso-SSH-senha-01','acme-ssh-chave-prod.passphrase':'falso-passphrase-01',
 'acme-ssh-salto.senha':'falso-SALTO-01','beta-github-api.token':'falso_token_github_01',
 'beta-aws-dev.secret':'falso-aws-secret-01',
 'beta-aws-dev.session_token':'falso-aws-session-01','beta-smtp-stage.senha':'falso-SMTP-01',
 'beta-sftp-dev.senha':'falso-SFTP-01','gama-nota.texto':'falso-nota-01','gama-generico.licenca':'falso-licenca-01',
 'gama-generico.anexo':'falso-anexo-01','gama-bloqueada.senha':'falso-BLOQ-01','delta-redir.senha':'falso-REDIR-01'}
# Identificadores falsos do contrato que não são segredo e podem aparecer (decisão 5 da
# Fase 10: o Access Key ID da AWS fica visível).
ident={'beta-aws-dev.access_key':'FALSOACCESSKEY01'}
raw=zipfile.ZipFile(F).read('document.json').decode()
hits={}
for nome,s in seg.items():
    formas={'literal':s,'url':urllib.parse.quote(s,safe=''),'base64':base64.b64encode(s.encode()).decode()}
    hits[nome]={'achado':any(v in raw for v in formas.values()),'formas':[f for f,v in formas.items() if v in raw]}
fs=set(re.findall(r'falso[-_][A-Za-z0-9:/@#_-]+|FALSO[A-Z0-9]+',raw))
conhecidos={**seg,**ident}
do_contrato=sorted(n for n,s in conhecidos.items() if s in fs)
R['V0.20']={'segredos_do_contrato_no_arquivo':hits,
 'falso_strings':{'quantidade':len(fs),'do_contrato':do_contrato,'fora_do_contrato':len(fs-set(conhecidos.values()))},
 'textos_unicos_layout':len(txt)}
# V0.22
md=open(MD).read()
ints=set(int(x) for x in re.findall(r'`(\d+)`',md))
R['V0.22']={'ids_citados':len(ints),'ids_ausentes':sorted(i for i in ints if str(i) not in N),
 'frames_sem_id_no_md':[k for k,i in top.items() if f'`{i}`' not in md],'componentes_sem_id_no_md':[k for k,o in comps.items() if f"`{o['id']}`" not in md]}
json.dump(R,open(OUT,'w'),ensure_ascii=False,indent=1)
for k,v in R.items():
    s=json.dumps(v,ensure_ascii=False); print(k, s[:600])
