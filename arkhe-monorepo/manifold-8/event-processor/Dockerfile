FROM python:3.11-slim

WORKDIR /app

# Dependências do serviço
COPY event-processor/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Código do pipeline
COPY src /app/src
COPY core /app/core
COPY verbal_events_processor.py /app/verbal_events_processor.py
COPY event-processor/app /app/app

ENV PYTHONUNBUFFERED=1 \
    SIMULATION_MODE=true \
    METRICS_PORT=8000

EXPOSE 8000

CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]