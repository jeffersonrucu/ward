// Comandos que a interface pode chamar. Espelho de `ward_app_nucleo::comandos::COMANDOS`
// (o teste `configuracao_tauri` do núcleo confere que as duas listas são iguais).
export const COMANDOS = Object.freeze(['estado_do_cofre', 'copiar_campo', 'listar_credenciais', 'detalhe_credencial', 'gerar_senha', 'copiar_senha_gerada', 'revelar_senha_gerada', 'descartar_senha_gerada', 'revelar_campo', 'codigo_totp', 'copiar_codigo_totp', 'usar_senha_gerada', 'descartar_formulario', 'salvar_credencial', 'mudar_nivel', 'escolher_anexo', 'remover_anexo', 'conectar', 'criar_cofre']);
