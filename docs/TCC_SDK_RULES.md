# Regras do SDK para o TCC

O SDK é a camada cliente de alto nível do Ponte Mesh.

Ele consulta o Origin, recebe access package, interpreta manifesto, seleciona fontes autorizadas, baixa fragmentos, valida integridade, preserva progresso e executa fallback automático.

## Inegociável

- Origin é autoridade.
- SDK não emite autorização.
- SDK não cria manifesto.
- SDK não usa API admin.
- SDK não usa MCP para download.
- SDK só usa authorizedSources.
- SDK valida hash antes de aceitar fragmento.
- SDK preserva fragmentos validados.
- SDK usa Origin como fallback final.
- P2P é aceleração, não autoridade.
- Replica/Edge é fonte auxiliar, não autoridade.
